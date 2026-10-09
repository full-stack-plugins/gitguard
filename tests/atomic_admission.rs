mod common;
use common::evidence::*;
use gitguard::execution::{
    ExecutionConfig, apply::*, grant::*, intent::IntentStore, platform_write::ProtectedBareTarget,
};
use guardengine::Enforcement;
use std::{collections::BTreeSet, sync::atomic::AtomicBool};
struct Clock;
impl GrantClock for Clock {
    fn adapter_id(&self) -> &str {
        "fixture"
    }
    fn now(&self) -> Result<i64, PortError> {
        Ok(20)
    }
}
struct Authority(OperationGrant);
impl GrantAuthority for Authority {
    fn adapter_id(&self) -> &str {
        "fixture"
    }
    fn authenticate(&self, _: &str) -> Result<OperationGrant, PortError> {
        Ok(self.0.clone())
    }
}
fn session(
    e: &Evidence,
    action: GrantAction,
    target: &str,
    operation_id: &str,
) -> (GrantSession, Authority) {
    let request = GrantRequest {
        actor: "fixture".into(),
        action,
        target: target.into(),
        expected_target_oid: if matches!(
            action,
            GrantAction::CreateBranch | GrantAction::CreateWorktree
        ) {
            None
        } else {
            Some(e.source.base.clone())
        },
        operation_id: operation_id.into(),
    };
    let authority = Authority(OperationGrant {
        api_version: "gitguard.operation-grant/v1alpha1".into(),
        issuer: "fixture".into(),
        actor: request.actor.clone(),
        action,
        repo_id: e.candidate.repo_id().into(),
        candidate_oid: e.candidate.candidate_oid().into(),
        binding_digest: e.candidate.binding_digest(),
        target: request.target.clone(),
        expected_target_oid: request.expected_target_oid.clone(),
        operation_id: request.operation_id.clone(),
        not_before: 0,
        expires_at: 100,
        revoked: false,
    });
    (
        GrantSession::new(
            GrantExpectation::freeze(
                &e.repo,
                &e.candidate,
                request,
                GrantPolicy {
                    authority_adapter: "fixture".into(),
                    clock_adapter: "fixture".into(),
                    allowed_issuers: BTreeSet::from(["fixture".into()]),
                    max_lifetime_seconds: 120,
                },
            )
            .unwrap(),
        ),
        authority,
    )
}
fn bare(e: &Evidence) -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    common::git(d.path(), &["init", "--bare", "-q"]);
    fn copy(from: &std::path::Path, to: &std::path::Path) {
        std::fs::create_dir_all(to).unwrap();
        for ent in std::fs::read_dir(from).unwrap() {
            let ent = ent.unwrap();
            if ent.file_type().unwrap().is_dir() {
                copy(&ent.path(), &to.join(ent.file_name()));
            } else {
                std::fs::copy(ent.path(), to.join(ent.file_name())).unwrap();
            }
        }
    }
    copy(
        &e.source.path().join(".git/objects"),
        &d.path().join("objects"),
    );
    common::git(d.path(), &["update-ref", "refs/heads/main", &e.source.base]);
    d
}
#[test]
fn explicit_local_atomic_apply_never_changes_the_source_or_replays() {
    let e = evidence(Enforcement::Advise, false, false);
    let before = e.source.state();
    let d = bare(&e);
    let marker = d.path().join("hook-ran");
    std::fs::write(
        d.path().join("hooks/reference-transaction"),
        format!("#!/bin/sh\necho unsafe > {}\n", marker.display()),
    )
    .unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            d.path().join("hooks/reference-transaction"),
            std::fs::Permissions::from_mode(0o700),
        )
        .unwrap();
    }
    let target = ProtectedBareTarget::open(d.path(), "repo").unwrap();
    let root = tempfile::Builder::new()
        .prefix(".apply-intent-")
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap();
    let intents = IntentStore::create(root.path()).unwrap();
    let (session, authority) = session(&e, GrantAction::Merge, "refs/heads/main", "operation");
    let cancel = AtomicBool::new(false);
    let config = ExecutionConfig { enabled: true };
    let context = ApplyContext {
        config: &config,
        repository: &e.repo,
        candidate: &e.candidate,
        session: &session,
        authority: &authority,
        clock: &Clock,
        grant_reference: "fixture:grant",
        intents: &intents,
        cancel: &cancel,
    };
    let result = apply(&context, &target);
    if cfg!(feature = "privileged-execution") {
        assert_eq!(result.unwrap().candidate_oid(), e.candidate.candidate_oid());
        assert_eq!(
            common::git(d.path(), &["rev-parse", "refs/heads/main"]),
            e.candidate.candidate_oid()
        );
        assert!(apply(&context, &target).is_err());
    } else {
        assert!(matches!(result, Err(ApplyError::Disabled)));
        assert_eq!(
            common::git(d.path(), &["rev-parse", "refs/heads/main"]),
            e.source.base
        );
    }
    assert!(!marker.exists());
    assert_eq!(e.source.state(), before);
}
#[cfg(feature = "privileged-execution")]
#[test]
fn default_config_unsupported_actions_and_revoked_authority_never_claim() {
    let e = evidence(Enforcement::Advise, false, false);
    let d = bare(&e);
    let target = ProtectedBareTarget::open(d.path(), "repo").unwrap();
    for action in [
        GrantAction::CreateBranch,
        GrantAction::CreateWorktree,
        GrantAction::Push,
        GrantAction::Merge,
    ] {
        let root = tempfile::Builder::new()
            .prefix(".apply-intent-")
            .tempdir_in(env!("CARGO_MANIFEST_DIR"))
            .unwrap();
        let intents = IntentStore::create(root.path()).unwrap();
        let before = std::fs::read(root.path().join("intent.json")).unwrap();
        let (session, mut authority) = session(
            &e,
            action,
            if action == GrantAction::CreateBranch {
                "refs/heads/new"
            } else if action == GrantAction::CreateWorktree {
                "worktrees/task"
            } else {
                "refs/heads/main"
            },
            "op",
        );
        authority.0.revoked = true;
        let cancel = AtomicBool::new(false);
        let config = ExecutionConfig::default();
        let mut context = ApplyContext {
            config: &config,
            repository: &e.repo,
            candidate: &e.candidate,
            session: &session,
            authority: &authority,
            clock: &Clock,
            grant_reference: "fixture",
            intents: &intents,
            cancel: &cancel,
        };
        assert!(matches!(
            apply(&context, &target),
            Err(ApplyError::Disabled)
        ));
        let enabled = ExecutionConfig { enabled: true };
        context.config = &enabled;
        assert!(matches!(
            apply(&context, &target),
            Err(ApplyError::Unsupported | ApplyError::Authority)
        ));
        assert_eq!(
            std::fs::read(root.path().join("intent.json")).unwrap(),
            before
        );
    }
}
#[cfg(feature = "privileged-execution")]
#[test]
fn branch_creation_and_stale_same_tree_commit_are_exact() {
    let e = evidence(Enforcement::Advise, false, false);
    let d = bare(&e);
    let target = ProtectedBareTarget::open(d.path(), "repo").unwrap();
    let root = tempfile::Builder::new()
        .prefix(".apply-intent-")
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap();
    let intents = IntentStore::create(root.path()).unwrap();
    let (session, authority) = session(&e, GrantAction::CreateBranch, "refs/heads/new", "op");
    let cancel = AtomicBool::new(false);
    let config = ExecutionConfig { enabled: true };
    let mut context = ApplyContext {
        config: &config,
        repository: &e.repo,
        candidate: &e.candidate,
        session: &session,
        authority: &authority,
        clock: &Clock,
        grant_reference: "fixture",
        intents: &intents,
        cancel: &cancel,
    };
    let tree = common::git(e.source.path(), &["rev-parse", "HEAD^{tree}"]);
    let different = common::git(
        e.source.path(),
        &[
            "commit-tree",
            &tree,
            "-p",
            &e.source.base,
            "-m",
            "same tree other commit",
        ],
    );
    assert_ne!(different, e.candidate.candidate_oid());
    let mut changed = serde_json::to_value(&e.candidate).unwrap();
    let refreshed = gitguard::Repository::discover(e.source.path(), "repo").unwrap();
    let alternate = refreshed
        .resolve_subject(gitguard::subject::SubjectRequest::Commit(different.clone()))
        .unwrap();
    changed["candidate_oid"] = different.into();
    changed["source_snapshot_digest"] = alternate.digest().into();
    let wrong: gitguard::candidate::CandidateSnapshot = serde_json::from_value(changed).unwrap();
    wrong.validate(&refreshed).unwrap();
    context.repository = &refreshed;
    context.candidate = &wrong;
    assert!(matches!(apply(&context, &target), Err(ApplyError::Binding)));
    context.candidate = &e.candidate;
    assert!(apply(&context, &target).is_ok());
    assert_eq!(
        common::git(d.path(), &["rev-parse", "refs/heads/new"]),
        e.candidate.candidate_oid()
    );
}
#[cfg(feature = "privileged-execution")]
#[test]
fn revocation_after_durable_claim_is_uncertain_and_never_retried() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Revokes {
        record: OperationGrant,
        calls: AtomicUsize,
    }
    impl GrantAuthority for Revokes {
        fn adapter_id(&self) -> &str {
            "fixture"
        }
        fn authenticate(&self, _: &str) -> Result<OperationGrant, PortError> {
            let mut r = self.record.clone();
            r.revoked = self.calls.fetch_add(1, Ordering::SeqCst) > 0;
            Ok(r)
        }
    }
    let e = evidence(Enforcement::Advise, false, false);
    let d = bare(&e);
    let target = ProtectedBareTarget::open(d.path(), "repo").unwrap();
    let root = tempfile::Builder::new()
        .prefix(".apply-intent-")
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap();
    let intents = IntentStore::create(root.path()).unwrap();
    let (session, good) = session(&e, GrantAction::Merge, "refs/heads/main", "op");
    let authority = Revokes {
        record: good.0.clone(),
        calls: AtomicUsize::new(0),
    };
    let cancel = AtomicBool::new(false);
    let config = ExecutionConfig { enabled: true };
    let mut context = ApplyContext {
        config: &config,
        repository: &e.repo,
        candidate: &e.candidate,
        session: &session,
        authority: &authority,
        clock: &Clock,
        grant_reference: "fixture",
        intents: &intents,
        cancel: &cancel,
    };
    assert!(matches!(
        apply(&context, &target),
        Err(ApplyError::RecoveryRequired)
    ));
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.path().join("intent.json")).unwrap()).unwrap();
    assert_eq!(json["events"][2]["state"], "recovery_required");
    context.authority = &good;
    assert!(matches!(apply(&context, &target), Err(ApplyError::Intent)));
    assert_eq!(
        common::git(d.path(), &["rev-parse", "refs/heads/main"]),
        e.source.base
    );
}
#[cfg(feature = "privileged-execution")]
#[test]
fn two_prechecked_writers_cannot_overwrite_the_cas_winner() {
    use std::sync::{
        Barrier,
        atomic::{AtomicUsize, Ordering},
    };
    struct Wait<'a> {
        record: OperationGrant,
        barrier: &'a Barrier,
        calls: AtomicUsize,
    }
    impl GrantAuthority for Wait<'_> {
        fn adapter_id(&self) -> &str {
            "fixture"
        }
        fn authenticate(&self, _: &str) -> Result<OperationGrant, PortError> {
            if self.calls.fetch_add(1, Ordering::SeqCst) == 1 {
                self.barrier.wait();
            }
            Ok(self.record.clone())
        }
    }
    let e = evidence(Enforcement::Advise, false, false);
    let d = bare(&e);
    let target = ProtectedBareTarget::open(d.path(), "repo").unwrap();
    let root = tempfile::Builder::new()
        .prefix(".apply-intent-")
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap();
    let intents = IntentStore::create(root.path()).unwrap();
    let root2 = tempfile::Builder::new()
        .prefix(".apply-intent-")
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap();
    let intents2 = IntentStore::create(root2.path()).unwrap();
    let (s1, a1) = session(&e, GrantAction::Merge, "refs/heads/main", "one");
    let (s2, a2) = session(&e, GrantAction::Merge, "refs/heads/main", "two");
    let barrier = Barrier::new(2);
    let a1 = Wait {
        record: a1.0,
        barrier: &barrier,
        calls: AtomicUsize::new(0),
    };
    let a2 = Wait {
        record: a2.0,
        barrier: &barrier,
        calls: AtomicUsize::new(0),
    };
    let cancel = AtomicBool::new(false);
    let config = ExecutionConfig { enabled: true };
    let (s1, s2, a1, a2) = (&s1, &s2, &a1, &a2);
    let results = std::thread::scope(|scope| {
        let run = |session, authority: &dyn GrantAuthority, intents: &IntentStore| {
            apply(
                &ApplyContext {
                    config: &config,
                    repository: &e.repo,
                    candidate: &e.candidate,
                    session,
                    authority,
                    clock: &Clock,
                    grant_reference: "fixture",
                    intents,
                    cancel: &cancel,
                },
                &target,
            )
            .is_ok()
        };
        let i1 = &intents;
        let i2 = &intents2;
        let x = scope.spawn(move || run(s1, a1, i1));
        let y = scope.spawn(move || run(s2, a2, i2));
        [x.join().unwrap(), y.join().unwrap()]
    });
    for root in [&root, &root2] {
        let j: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.path().join("intent.json")).unwrap())
                .unwrap();
        assert_eq!(j["events"][1]["state"], "attempt_recorded");
    }
    assert_eq!(results.into_iter().filter(|b| *b).count(), 1);
    assert_eq!(
        common::git(d.path(), &["rev-parse", "refs/heads/main"]),
        e.candidate.candidate_oid()
    );
}
#[test]
fn target_config_paths_hooks_and_objects_have_explicit_limits() {
    use std::os::unix::fs::PermissionsExt;
    let e = evidence(Enforcement::Advise, false, false);
    for case in 0..5 {
        let d = bare(&e);
        match case {
            0 => std::fs::write(d.path().join("objects/info/alternates"), "/tmp/other").unwrap(),
            1 => {
                let p = d.path().join("config");
                let mut s = std::fs::read_to_string(&p).unwrap();
                s.push_str("\n[include]\npath=/tmp/other\n");
                std::fs::write(p, s).unwrap();
            }
            2 => std::os::unix::fs::symlink("/tmp", d.path().join("refs/heads/escape")).unwrap(),
            3 => {
                std::fs::set_permissions(d.path(), std::fs::Permissions::from_mode(0o755)).unwrap()
            }
            _ => {
                let f = std::fs::File::create(d.path().join("oversized")).unwrap();
                f.set_len(65 * 1024 * 1024).unwrap();
            }
        }
        assert!(
            ProtectedBareTarget::open(d.path(), "repo").is_err(),
            "case {case}"
        );
    }
}
#[cfg(feature = "privileged-execution")]
#[test]
fn non_ancestor_old_commit_is_never_force_updated() {
    let mut e = evidence(Enforcement::Advise, false, false);
    let tree = common::git(e.source.path(), &["rev-parse", "HEAD^{tree}"]);
    let unrelated = common::git(
        e.source.path(),
        &["commit-tree", &tree, "-m", "unrelated root"],
    );
    e.source.base = unrelated.clone();
    let d = bare(&e);
    let target = ProtectedBareTarget::open(d.path(), "repo").unwrap();
    let root = tempfile::Builder::new()
        .prefix(".apply-intent-")
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap();
    let intents = IntentStore::create(root.path()).unwrap();
    let initial = std::fs::read(root.path().join("intent.json")).unwrap();
    let (session, authority) = session(&e, GrantAction::Merge, "refs/heads/main", "op");
    let cancel = AtomicBool::new(false);
    let config = ExecutionConfig { enabled: true };
    let context = ApplyContext {
        config: &config,
        repository: &e.repo,
        candidate: &e.candidate,
        session: &session,
        authority: &authority,
        clock: &Clock,
        grant_reference: "fixture",
        intents: &intents,
        cancel: &cancel,
    };
    assert!(matches!(apply(&context, &target), Err(ApplyError::Target)));
    assert_eq!(
        common::git(d.path(), &["rev-parse", "refs/heads/main"]),
        unrelated
    );
    assert_eq!(
        std::fs::read(root.path().join("intent.json")).unwrap(),
        initial
    );
}
#[cfg(feature = "privileged-execution")]
#[test]
fn prepared_operation_cannot_be_redirected_to_another_bare_directory() {
    struct Cancel<'a> {
        record: OperationGrant,
        cancel: &'a AtomicBool,
    }
    impl GrantAuthority for Cancel<'_> {
        fn adapter_id(&self) -> &str {
            "fixture"
        }
        fn authenticate(&self, _: &str) -> Result<OperationGrant, PortError> {
            self.cancel.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(self.record.clone())
        }
    }
    let e = evidence(Enforcement::Advise, false, false);
    let first = bare(&e);
    let second = bare(&e);
    let target = ProtectedBareTarget::open(first.path(), "repo").unwrap();
    let other = ProtectedBareTarget::open(second.path(), "repo").unwrap();
    let root = tempfile::Builder::new()
        .prefix(".apply-intent-")
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap();
    let intents = IntentStore::create(root.path()).unwrap();
    let (session, good) = session(&e, GrantAction::Merge, "refs/heads/main", "op");
    let cancel = AtomicBool::new(false);
    let cancelling = Cancel {
        record: good.0.clone(),
        cancel: &cancel,
    };
    let config = ExecutionConfig { enabled: true };
    let mut context = ApplyContext {
        config: &config,
        repository: &e.repo,
        candidate: &e.candidate,
        session: &session,
        authority: &cancelling,
        clock: &Clock,
        grant_reference: "fixture",
        intents: &intents,
        cancel: &cancel,
    };
    assert!(matches!(
        apply(&context, &target),
        Err(ApplyError::Cancelled)
    ));
    cancel.store(false, std::sync::atomic::Ordering::SeqCst);
    context.authority = &good;
    assert!(matches!(apply(&context, &other), Err(ApplyError::Intent)));
    assert_eq!(
        common::git(second.path(), &["rev-parse", "refs/heads/main"]),
        e.source.base
    );
    assert!(apply(&context, &target).is_ok());
}
