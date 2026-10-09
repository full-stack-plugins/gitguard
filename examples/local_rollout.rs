//! Self-contained local rollout fixture; accepts no user repository or remote destination.
//! Captured Gitflow observations and fixture grant authority are not production certification.
use gitguard::{
    Repository,
    candidate::{CandidateRequest, CandidateSnapshot},
    cli::CheckRequest,
    evidence::{
        envelope::{BoundCheck, CheckBundle},
        projection::FrozenPolicy,
    },
    execution::{
        ExecutionConfig,
        apply::{ApplyContext, ApplyError, apply},
        grant::*,
        intent::IntentStore,
        platform_write::ProtectedBareTarget,
        reconcile::{RecoveryState, reconcile},
    },
    gitflow::{ExpectedWorkflow, NativeOutput, Outcome, PROFILE, PreparedCheck},
    scope::TaskScope,
};
use guardengine::{Decision, Enforcement};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    os::unix::fs::PermissionsExt,
    path::Path,
    process::Command,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("/usr/bin/git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", root)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .output()
        .expect("fixture git");
    assert!(output.status.success(), "fixture git failed");
    String::from_utf8(output.stdout).unwrap().trim().into()
}
fn private(root: &Path) {
    std::fs::create_dir(root).unwrap();
    std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700)).unwrap();
}
fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            copy(&entry.path(), &to.join(entry.file_name()));
        } else {
            std::fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
        }
    }
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn source_digest(root: &Path) -> String {
    fn scan(root: &Path, here: &Path, entries: &mut Vec<(Vec<u8>, Vec<u8>)>) {
        for entry in std::fs::read_dir(here).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                scan(root, &entry.path(), entries);
            } else {
                entries.push((
                    entry
                        .path()
                        .strip_prefix(root)
                        .unwrap()
                        .as_os_str()
                        .as_encoded_bytes()
                        .to_vec(),
                    std::fs::read(entry.path()).unwrap(),
                ));
            }
        }
    }
    let mut entries = vec![];
    scan(root, root, &mut entries);
    entries.sort();
    digest(&serde_json::to_vec(&entries).unwrap())
}
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
    calls: AtomicUsize,
    revoke_after_claim: bool,
}
impl GrantAuthority for Authority {
    fn adapter_id(&self) -> &str {
        "fixture-authority"
    }
    fn authenticate(&self, reference: &str) -> Result<OperationGrant, PortError> {
        if reference != "fixture:grant" {
            return Err(PortError::Untrusted);
        }
        let mut grant = self.record.clone();
        if self.revoke_after_claim && self.calls.fetch_add(1, Ordering::SeqCst) > 0 {
            grant.revoked = true;
        }
        Ok(grant)
    }
}
fn session(
    repo: &Repository,
    candidate: &CandidateSnapshot,
    operation: &str,
    branch: &str,
    revoke: bool,
) -> (GrantSession, Authority) {
    let request = GrantRequest {
        actor: "fixture".into(),
        action: GrantAction::Push,
        target: format!("refs/heads/{branch}"),
        expected_target_oid: Some(candidate.base_oid().into()),
        operation_id: operation.into(),
    };
    let authority = Authority {
        record: OperationGrant {
            api_version: "gitguard.operation-grant/v1alpha1".into(),
            issuer: "fixture".into(),
            actor: request.actor.clone(),
            action: request.action,
            repo_id: candidate.repo_id().into(),
            candidate_oid: candidate.candidate_oid().into(),
            binding_digest: candidate.binding_digest(),
            target: request.target.clone(),
            expected_target_oid: request.expected_target_oid.clone(),
            operation_id: operation.into(),
            not_before: 0,
            expires_at: 100,
            revoked: false,
        },
        calls: AtomicUsize::new(0),
        revoke_after_claim: revoke,
    };
    let expected = GrantExpectation::freeze(
        repo,
        candidate,
        request,
        GrantPolicy {
            authority_adapter: "fixture-authority".into(),
            clock_adapter: "fixture-clock".into(),
            allowed_issuers: BTreeSet::from(["fixture".into()]),
            max_lifetime_seconds: 120,
        },
    )
    .unwrap();
    (GrantSession::new(expected), authority)
}
fn native(
    repo: &Repository,
    candidate: &CandidateSnapshot,
    branch: &str,
    metadata: &Value,
) -> PreparedCheck {
    PreparedCheck::freeze(
        repo,
        candidate,
        ExpectedWorkflow {
            profile: PROFILE.into(),
            source: "feature/task".into(),
            target: branch.into(),
            policy_ref: candidate.base_oid().into(),
            raw_policy_digest: metadata["policy_sha256"].as_str().unwrap().into(),
            native_policy_digest:
                "890bd93513538e41318f9a5d23e68036252c11f2ce2849aa84fd766217164378".into(),
        },
        include_bytes!("../adapters/gitflow/fixtures/workflow.json"),
    )
    .unwrap()
}
/// Private demonstration-controller observation; never serialized into an authorization token.
struct Shadow {
    active: bool,
    compatible: bool,
    agrees: bool,
}
fn shadow(
    adapter: Option<&PreparedCheck>,
    candidate: &CandidateSnapshot,
    bundle: &CheckBundle,
    code: i32,
    raw: &[u8],
) -> Shadow {
    match adapter {
        None => Shadow {
            active: false,
            compatible: false,
            agrees: false,
        },
        Some(adapter) => {
            let observation = adapter
                .consume(candidate, NativeOutput::Exited { code, stdout: raw })
                .unwrap();
            let agrees = matches!(
                (observation.outcome(), bundle.envelope.decision.as_ref()),
                (Outcome::Compatible, Some(Decision::Allow))
                    | (Outcome::Denied, Some(Decision::Block))
            );
            Shadow {
                active: true,
                compatible: observation.outcome() == Outcome::Compatible,
                agrees,
            }
        }
    }
}
fn opt_in(comparison: &Shadow) -> ExecutionConfig {
    // This controller has no adjudication for mismatches: never turn them into write enablement.
    ExecutionConfig {
        enabled: comparison.active && comparison.compatible && comparison.agrees,
    }
}

pub fn run() -> Result<Value, String> {
    if !cfg!(feature = "privileged-execution") {
        return Err("privileged execution fixture is not enabled".into());
    }
    let fixture = tempfile::Builder::new()
        .prefix(".local-rollout-")
        .permissions(std::fs::Permissions::from_mode(0o700))
        .tempdir_in(std::env::current_dir().map_err(|_| "working directory unavailable")?)
        .map_err(|_| "fixture unavailable")?;
    let source = fixture.path().join("source");
    private(&source);
    git(
        &source,
        &[
            "clone",
            "-q",
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/adapters/gitflow/fixtures/source.bundle"
            ),
            ".",
        ],
    );
    let before = source_digest(&source);
    let metadata: Value =
        serde_json::from_str(include_str!("../adapters/gitflow/fixtures/capture.json")).unwrap();
    let policy = FrozenPolicy::new(Enforcement::Advise);
    let bundle = BoundCheck::prepare(CheckRequest {
        api_version: "gitguard.check/v1alpha1".into(),
        repo_root: source.clone(),
        repo_id: "fixture-repo".into(),
        candidate_oid: metadata["candidate_oid"].as_str().unwrap().into(),
        scope: TaskScope::advisory(
            "rollout-task",
            vec!["R".into()],
            vec![b"a".to_vec()],
            &policy.digest(),
            None,
        )
        .unwrap(),
        candidate: CandidateRequest {
            worktree_id: "fixture-worktree".into(),
            base_oid: metadata["base_oid"].as_str().unwrap().into(),
            merge_group_id: None,
            members: vec![],
        },
        contract: policy.contract(),
    })
    .map_err(|_| "check preparation failed")?
    .run(&AtomicBool::new(false))
    .map_err(|_| "check failed")?;
    let repo =
        Repository::discover(&source, "fixture-repo").map_err(|_| "repository unavailable")?;
    let candidate: CandidateSnapshot =
        serde_json::from_value(bundle.domain["candidate"].clone()).unwrap();
    assert_eq!(bundle.exit_code(), 0);
    let mut adapter = Some(native(&repo, &candidate, "develop", &metadata));
    let deny_adapter = native(&repo, &candidate, "main", &metadata);
    let allow = include_bytes!("../adapters/gitflow/fixtures/allow.stdout");
    let deny = include_bytes!("../adapters/gitflow/fixtures/deny-target.stdout");
    let comparison = shadow(adapter.as_ref(), &candidate, &bundle, 0, allow);
    let mismatch = shadow(Some(&deny_adapter), &candidate, &bundle, 1, deny);
    assert!(comparison.agrees);
    assert!(!mismatch.agrees);
    let bare = fixture.path().join("target");
    private(&bare);
    git(&bare, &["init", "--bare", "-q"]);
    copy(&source.join(".git/objects"), &bare.join("objects"));
    git(
        &bare,
        &["update-ref", "refs/heads/develop", candidate.base_oid()],
    );
    git(
        &bare,
        &["update-ref", "refs/heads/main", candidate.base_oid()],
    );
    let target = ProtectedBareTarget::open(&bare, "fixture-repo").unwrap();
    let intent_root = fixture.path().join("intents");
    private(&intent_root);
    let intents = IntentStore::create(&intent_root).unwrap();
    let empty_history = std::fs::read(intent_root.join("intent.json")).unwrap();
    let (publish, publish_authority) = session(&repo, &candidate, "publish", "develop", false);
    let cancel = AtomicBool::new(false);
    let disabled = ExecutionConfig::default();
    let mut context = ApplyContext {
        config: &disabled,
        repository: &repo,
        candidate: &candidate,
        session: &publish,
        authority: &publish_authority,
        clock: &Clock,
        grant_reference: "fixture:grant",
        intents: &intents,
        cancel: &cancel,
    };
    assert!(matches!(
        apply(&context, &target),
        Err(ApplyError::Disabled)
    ));
    assert_eq!(
        std::fs::read(intent_root.join("intent.json")).unwrap(),
        empty_history
    );
    let unresolved = opt_in(&mismatch);
    context.config = &unresolved;
    assert!(matches!(
        apply(&context, &target),
        Err(ApplyError::Disabled)
    ));
    assert_eq!(
        git(&bare, &["rev-parse", "refs/heads/develop"]),
        candidate.base_oid()
    );
    let enabled = opt_in(&comparison);
    context.config = &enabled;
    let receipt = apply(&context, &target).map_err(|_| "explicit local write failed")?;
    let actual_target = git(&bare, &["rev-parse", "refs/heads/develop"]);
    assert_eq!(actual_target, candidate.candidate_oid());
    // A distinct real claimed attempt becomes uncertain; rollback must preserve it too.
    let (uncertain, uncertain_authority) = session(&repo, &candidate, "uncertain", "main", true);
    context.session = &uncertain;
    context.authority = &uncertain_authority;
    assert!(matches!(
        apply(&context, &target),
        Err(ApplyError::RecoveryRequired)
    ));
    assert_eq!(
        git(&bare, &["rev-parse", "refs/heads/main"]),
        candidate.base_oid()
    );
    let preserved = std::fs::read(intent_root.join("intent.json")).unwrap();
    // Adapter rollback independently stops scheduling; existing executor configuration is untouched.
    adapter = None;
    let stopped = shadow(adapter.as_ref(), &candidate, &bundle, 0, allow);
    assert!(!stopped.active);
    assert!(!opt_in(&stopped).enabled);
    assert!(enabled.enabled);
    let adapter_off = opt_in(&stopped);
    context.config = &adapter_off;
    assert!(matches!(
        apply(&context, &target),
        Err(ApplyError::Disabled)
    ));
    assert_eq!(
        std::fs::read(intent_root.join("intent.json")).unwrap(),
        preserved
    );
    // Executor rollback independently rejects even a new valid grant, while shadow remains available.
    adapter = Some(native(&repo, &candidate, "develop", &metadata));
    let available = shadow(adapter.as_ref(), &candidate, &bundle, 0, allow);
    let (fresh, fresh_authority) = session(&repo, &candidate, "disabled-new", "main", false);
    context.session = &fresh;
    context.authority = &fresh_authority;
    context.config = &disabled;
    assert!(matches!(
        apply(&context, &target),
        Err(ApplyError::Disabled)
    ));
    assert!(available.agrees);
    drop(intents);
    let reopened = IntentStore::open(&intent_root).unwrap();
    assert!(matches!(
        reconcile(&publish, &reopened, &target).unwrap(),
        RecoveryState::Applied { .. }
    ));
    assert_eq!(
        reconcile(&uncertain, &reopened, &target).unwrap(),
        RecoveryState::RecoveryRequired
    );
    let history_after = std::fs::read(intent_root.join("intent.json")).unwrap();
    assert_eq!(history_after, preserved);
    let after = source_digest(&source);
    assert_eq!(before, after);
    Ok(
        json!({ "profile": "isolated-local-rollout-fixture", "production_authority": false,
        "native_observation": "pinned captured Gitflow output, not a live hosted check", "candidate_oid": candidate.candidate_oid(),
        "native_profile": PROFILE, "source_before": before, "source_after": after, "source_unchanged": before == after,
        "advisory": {"decision": bundle.envelope.decision, "write": "Disabled"},
        "shadow": {"comparison": "agree", "native_digest": digest(allow), "gg_exit": bundle.exit_code()},
        "unresolved_difference": {"native": "Denied", "gitguard": "ALLOW", "enabled": unresolved.enabled, "write": "Disabled"},
        "opt_in": {"observed_target": actual_target, "operation_id": receipt.operation_id(), "request_digest": receipt.request_digest()},
        "adapter_rollback": {"scheduled_write": stopped.active, "executor_config_retained": enabled.enabled},
        "executor_rollback": {"write": "Disabled", "shadow_still_available": available.agrees},
        "history_unchanged_after_rollback": history_after == preserved, "history_digest": digest(&history_after),
        "ack_after_rollback": "Applied", "unknown_after_rollback": "RecoveryRequired" }),
    )
}
fn main() {
    if std::env::args_os().len() != 1 {
        std::process::exit(4);
    }
    match run() {
        Ok(report) => println!("{}", serde_json::to_string_pretty(&report).unwrap()),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(4);
        }
    }
}
