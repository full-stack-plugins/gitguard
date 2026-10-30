//! Isolated fixture demonstration, not a production identity provider or user-repository CLI.
use gitguard::{
    Repository,
    candidate::{CandidateRequest, CandidateSnapshot},
    execution::{
        ExecutionConfig,
        apply::{ApplyContext, ApplyError, LocalCasReceipt, apply},
        grant::*,
        intent::IntentStore,
        platform_write::ProtectedBareTarget,
    },
    scope::TaskScope,
    subject::SubjectRequest,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc, Barrier,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
const REFERENCE: &str = "fixture:grant";
struct Clock;
impl GrantClock for Clock {
    fn adapter_id(&self) -> &str {
        "fixture-clock"
    }
    fn now(&self) -> Result<i64, PortError> {
        Ok(20)
    }
}
struct Authority {
    record: OperationGrant,
    denied: bool,
    barrier: Option<Arc<Barrier>>,
    calls: AtomicUsize,
}
impl GrantAuthority for Authority {
    fn adapter_id(&self) -> &str {
        "fixture-authority"
    }
    fn authenticate(&self, reference: &str) -> Result<OperationGrant, PortError> {
        if self.denied || reference != REFERENCE {
            return Err(PortError::Untrusted);
        }
        if self.calls.fetch_add(1, Ordering::SeqCst) == 1
            && let Some(barrier) = &self.barrier
        {
            barrier.wait();
        }
        Ok(self.record.clone())
    }
}
fn git_output(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new("/usr/bin/git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", root)
        .env("LC_ALL", "C")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_AUTHOR_NAME", "Fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
        .env("GIT_COMMITTER_NAME", "Fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
        .env("GIT_AUTHOR_DATE", "2000-01-01T00:00:00Z")
        .env("GIT_COMMITTER_DATE", "2000-01-01T00:00:00Z")
        .output()
        .expect("fixture Git must start")
}
fn git(root: &Path, args: &[&str]) -> String {
    let o = git_output(root, args);
    assert!(
        o.status.success(),
        "fixture Git failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    String::from_utf8(o.stdout).unwrap().trim().into()
}
fn private_dir(path: &Path) {
    std::fs::DirBuilder::new().mode(0o700).create(path).unwrap();
}
fn copy_objects(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        if e.file_type().unwrap().is_dir() {
            copy_objects(&e.path(), &to.join(e.file_name()));
        } else {
            assert!(e.file_type().unwrap().is_file());
            std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
        }
    }
}
fn digest_tree(root: &Path) -> String {
    fn walk(root: &Path, dir: &Path, map: &mut BTreeMap<String, String>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let e = e.unwrap();
            if e.file_type().unwrap().is_dir() {
                walk(root, &e.path(), map);
            } else {
                map.insert(
                    e.path()
                        .strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .into(),
                    format!("{:x}", Sha256::digest(std::fs::read(e.path()).unwrap())),
                );
            }
        }
    }
    let mut files = BTreeMap::new();
    walk(root, root, &mut files);
    format!("{:x}", Sha256::digest(serde_json::to_vec(&files).unwrap()))
}
struct Fixture {
    _root: tempfile::TempDir,
    source: PathBuf,
    repo: Repository,
    candidate: CandidateSnapshot,
    alternate: CandidateSnapshot,
    base: String,
    unrelated: String,
}
fn candidate(repo: &Repository, oid: &str, base: &str) -> CandidateSnapshot {
    repo.prepare_candidate(
        &repo
            .resolve_subject(SubjectRequest::Commit(oid.into()))
            .unwrap(),
        &TaskScope::advisory(
            "fixture-task",
            vec!["FIXTURE-1".into()],
            vec![b"a".to_vec()],
            &"a".repeat(64),
            None,
        )
        .unwrap(),
        &CandidateRequest {
            worktree_id: "fixture-worktree".into(),
            base_oid: base.into(),
            merge_group_id: None,
            members: vec![],
        },
    )
    .unwrap()
}
impl Fixture {
    fn new() -> Self {
        // No path argument: only a newly allocated private child, never an existing user repository.
        let root = tempfile::Builder::new()
            .prefix(".gitguard-local-apply-example-")
            .tempdir_in(std::env::current_dir().unwrap())
            .unwrap();
        let source = root.path().join("source");
        private_dir(&source);
        git(&source, &["init", "-q", "--object-format=sha1"]);
        std::fs::write(source.join("a"), "base\n").unwrap();
        git(&source, &["add", "a"]);
        git(&source, &["commit", "-qm", "base"]);
        let base = git(&source, &["rev-parse", "HEAD"]);
        std::fs::write(source.join("a"), "candidate\n").unwrap();
        git(&source, &["commit", "-qam", "feat: checked candidate"]);
        let head = git(&source, &["rev-parse", "HEAD"]);
        let tree = git(&source, &["rev-parse", "HEAD^{tree}"]);
        let other = git(
            &source,
            &[
                "commit-tree",
                &tree,
                "-p",
                &base,
                "-m",
                "same tree different commit",
            ],
        );
        let unrelated = git(&source, &["commit-tree", &tree, "-m", "unrelated root"]);
        let repo = Repository::discover(&source, "fixture-repo").unwrap();
        let frozen = candidate(&repo, &head, &base);
        let alternate = candidate(&repo, &other, &base);
        alternate.validate(&repo).unwrap();
        Self {
            _root: root,
            source,
            repo,
            candidate: frozen,
            alternate,
            base,
            unrelated,
        }
    }
}
struct Case {
    _root: tempfile::TempDir,
    bare: PathBuf,
    intent_path: PathBuf,
    target: ProtectedBareTarget,
    intents: IntentStore,
    refname: &'static str,
}
impl Case {
    fn new(f: &Fixture, old: Option<&str>) -> Self {
        let root = tempfile::Builder::new()
            .prefix("case-")
            .tempdir_in(f._root.path())
            .unwrap();
        let bare = root.path().join("bare");
        private_dir(&bare);
        git(&bare, &["init", "--bare", "-q", "--object-format=sha1"]);
        copy_objects(&f.source.join(".git/objects"), &bare.join("objects"));
        let refname = "refs/heads/fixture-target";
        if let Some(old) = old {
            git(&bare, &["update-ref", refname, old, &"0".repeat(40)]);
        }
        let intent_path = root.path().join("intent");
        private_dir(&intent_path);
        let intents = IntentStore::create(&intent_path).unwrap();
        let target = ProtectedBareTarget::open(&bare, "fixture-repo").unwrap();
        Self {
            _root: root,
            bare,
            intent_path,
            target,
            intents,
            refname,
        }
    }
    fn oid(&self) -> Option<String> {
        let out = git_output(&self.bare, &["rev-parse", "--verify", self.refname]);
        out.status
            .success()
            .then(|| String::from_utf8(out.stdout).unwrap().trim().into())
    }
}
fn events(path: &Path) -> Value {
    let s: Value =
        serde_json::from_slice(&std::fs::read(path.join("intent.json")).unwrap()).unwrap();
    s["events"].clone()
}
fn authority(
    f: &Fixture,
    c: &Case,
    action: GrantAction,
    old: Option<&str>,
    id: &str,
) -> (GrantSession, Authority, Value) {
    let request = GrantRequest {
        actor: "fixture-controller".into(),
        action,
        target: c.refname.into(),
        expected_target_oid: old.map(str::to_owned),
        operation_id: id.into(),
    };
    let record = OperationGrant {
        api_version: "gitguard.operation-grant/v1alpha1".into(),
        issuer: "fixture-only-issuer".into(),
        actor: request.actor.clone(),
        action,
        repo_id: "fixture-repo".into(),
        candidate_oid: f.candidate.candidate_oid().into(),
        binding_digest: f.candidate.binding_digest(),
        target: request.target.clone(),
        expected_target_oid: request.expected_target_oid.clone(),
        operation_id: id.into(),
        not_before: 0,
        expires_at: 100,
        revoked: false,
    };
    let inputs = json!({"request":request,"grant_reference":REFERENCE,"grant":record,"trusted_fixture_time":20});
    let expected = GrantExpectation::freeze(
        &f.repo,
        &f.candidate,
        request,
        GrantPolicy {
            authority_adapter: "fixture-authority".into(),
            clock_adapter: "fixture-clock".into(),
            allowed_issuers: BTreeSet::from(["fixture-only-issuer".into()]),
            max_lifetime_seconds: 120,
        },
    )
    .unwrap();
    (
        GrantSession::new(expected),
        Authority {
            record,
            denied: false,
            barrier: None,
            calls: AtomicUsize::new(0),
        },
        inputs,
    )
}
fn dispatch(
    f: &Fixture,
    candidate: &CandidateSnapshot,
    target: &ProtectedBareTarget,
    intents: &IntentStore,
    session: &GrantSession,
    authority: &Authority,
) -> Result<LocalCasReceipt, ApplyError> {
    apply(
        &ApplyContext {
            config: &ExecutionConfig { enabled: true },
            repository: &f.repo,
            candidate,
            session,
            authority,
            clock: &Clock,
            grant_reference: REFERENCE,
            intents,
            cancel: &AtomicBool::new(false),
        },
        target,
    )
}
fn outcome(result: Result<LocalCasReceipt, ApplyError>) -> Value {
    match result {
        Ok(r) => {
            json!({"status":"local_cas_acknowledged","candidate_oid":r.candidate_oid(),"operation_id":r.operation_id(),"request_digest":r.request_digest(),"reconciled":false})
        }
        Err(e) => json!({"status":"rejected","error":format!("{e:?}"),"reconciled":false}),
    }
}
fn main() {
    if std::env::args_os().len() != 1 || !cfg!(feature = "privileged-execution") {
        println!(
            "{}",
            json!({"error":"requires privileged-execution feature and no arguments","user_repository_paths_accepted":false})
        );
        std::process::exit(4);
    }
    let f = Fixture::new();
    let before_source = digest_tree(&f.source);
    let mut cases = vec![];
    for name in [
        "create_branch",
        "fast_forward_merge",
        "non_fast_forward",
        "changed_candidate",
        "permission_denied",
    ] {
        let old = if name == "create_branch" {
            None
        } else if name == "non_fast_forward" {
            Some(f.unrelated.as_str())
        } else {
            Some(f.base.as_str())
        };
        let c = Case::new(&f, old);
        let action = if name == "create_branch" {
            GrantAction::CreateBranch
        } else {
            GrantAction::Merge
        };
        let before = c.oid();
        let (session, mut auth, inputs) = authority(&f, &c, action, old, name);
        auth.denied = name == "permission_denied";
        let submitted = if name == "changed_candidate" {
            &f.alternate
        } else {
            &f.candidate
        };
        let result = dispatch(&f, submitted, &c.target, &c.intents, &session, &auth);
        let expected = match name {
            "non_fast_forward" => Some(ApplyError::Target),
            "changed_candidate" => Some(ApplyError::Binding),
            "permission_denied" => Some(ApplyError::Authority),
            _ => None,
        };
        if let Some(error) = expected {
            assert!(matches!(&result,Err(e) if *e==error));
            assert_eq!(c.oid(), before);
        } else {
            assert!(result.is_ok());
            assert_eq!(c.oid().as_deref(), Some(f.candidate.candidate_oid()));
        }
        cases.push(json!({"case":name,"inputs":inputs,"submitted_candidate_oid":submitted.candidate_oid(),"before_oid":before,"after_oid":c.oid(),"result":outcome(result),"intent_events":events(&c.intent_path)}));
    }
    let c = Case::new(&f, Some(&f.base));
    let second = c._root.path().join("second-intent");
    private_dir(&second);
    let other_intents = IntentStore::create(&second).unwrap();
    let (s1, mut a1, input1) = authority(&f, &c, GrantAction::Merge, Some(&f.base), "atomic-a");
    let (s2, mut a2, input2) = authority(&f, &c, GrantAction::Merge, Some(&f.base), "atomic-b");
    let barrier = Arc::new(Barrier::new(2));
    a1.barrier = Some(barrier.clone());
    a2.barrier = Some(barrier);
    let before = c.oid();
    let (r1, r2) = std::thread::scope(|scope| {
        let x = scope.spawn(|| dispatch(&f, &f.candidate, &c.target, &c.intents, &s1, &a1));
        let y = scope.spawn(|| dispatch(&f, &f.candidate, &c.target, &other_intents, &s2, &a2));
        (x.join().unwrap(), y.join().unwrap())
    });
    assert_ne!(r1.is_ok(), r2.is_ok());
    assert!(matches!(&r1, Ok(_) | Err(ApplyError::RecoveryRequired)));
    assert!(matches!(&r2, Ok(_) | Err(ApplyError::RecoveryRequired)));
    assert_eq!(c.oid().as_deref(), Some(f.candidate.candidate_oid()));
    cases.push(json!({"case":"atomic_competition","inputs":[input1,input2],"before_oid":before,"after_oid":c.oid(),"results":[outcome(r1),outcome(r2)],"intent_events":[events(&c.intent_path),events(&second)],"barrier":"after both durable claims, before two real Git CAS commands"}));
    let after_source = digest_tree(&f.source);
    assert_eq!(before_source, after_source);
    println!("{}",serde_json::to_string_pretty(&json!({"profile":"isolated-local-bare-fixture/v1","fixture_identity":"fixture-only-issuer; not production authentication","git_version":git(&f.source,&["version"]),"fixed_commit_date":"2000-01-01T00:00:00Z","base_oid":f.base,"candidate":f.candidate,"same_tree_alternate_oid":f.alternate.candidate_oid(),"source_before_sha256":before_source,"source_after_sha256":after_source,"source_unchanged":true,"temporary_repositories_removed_on_exit":true,"receipt_semantics":"historical local CAS acknowledgement; reconciliation not implemented","cases":cases})).unwrap());
}
