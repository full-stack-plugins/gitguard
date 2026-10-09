#![cfg(feature = "privileged-execution")]
mod common;
use common::evidence::*;
use gitguard::execution::{
    ExecutionConfig, apply::*, grant::*, intent::IntentStore, platform_write::ProtectedBareTarget,
};
use guardengine::Enforcement;
use std::{collections::BTreeSet, os::unix::fs::PermissionsExt, sync::atomic::AtomicBool};
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
    let d = tempfile::Builder::new()
        .permissions(std::fs::Permissions::from_mode(0o700))
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap();
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

use gitguard::execution::reconcile::{RecoveryState, reconcile};
#[test]
fn durable_ack_survives_reopen_and_target_advance_but_never_satisfies_another_operation() {
    let e = evidence(Enforcement::Advise, false, false);
    let d = bare(&e);
    let target = ProtectedBareTarget::open(d.path(), "repo").unwrap();
    let root = tempfile::Builder::new()
        .permissions(std::fs::Permissions::from_mode(0o700))
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap();
    let intents = IntentStore::create(root.path()).unwrap();
    let (session, authority) = session(&e, GrantAction::Push, "refs/heads/main", "acknowledged");
    let config = ExecutionConfig { enabled: true };
    let cancel = AtomicBool::new(false);
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
    let applied = apply(&context, &target).unwrap();
    drop(intents);
    let tree = common::git(
        d.path(),
        &[
            "rev-parse",
            &format!("{}^{{tree}}", e.candidate.candidate_oid()),
        ],
    );
    let later = common::git(
        d.path(),
        &[
            "commit-tree",
            &tree,
            "-p",
            e.candidate.candidate_oid(),
            "-m",
            "later",
        ],
    );
    common::git(d.path(), &["update-ref", "refs/heads/main", &later]);
    let reopened = IntentStore::open(root.path()).unwrap();
    let bytes_before = std::fs::read(root.path().join("intent.json")).unwrap();
    assert_eq!(
        reconcile(&session, &reopened, &target).unwrap(),
        RecoveryState::Applied {
            operation_id: applied.operation_id().into(),
            request_digest: applied.request_digest().into(),
            candidate_oid: applied.candidate_oid().into()
        }
    );
    let (other, _) = self::session(&e, GrantAction::Push, "refs/heads/main", "another");
    assert_eq!(
        reconcile(&other, &reopened, &target).unwrap(),
        RecoveryState::NotRecorded
    );
    let second = bare(&e);
    let other_target = ProtectedBareTarget::open(second.path(), "repo").unwrap();
    assert!(reconcile(&session, &reopened, &other_target).is_err());
    assert_eq!(
        std::fs::read(root.path().join("intent.json")).unwrap(),
        bytes_before
    );
    assert_eq!(
        common::git(d.path(), &["rev-parse", "refs/heads/main"]),
        later
    );
}

#[test]
fn prepared_without_attempt_is_retryable_only_through_fresh_authority_validation() {
    use std::sync::atomic::Ordering;
    struct CancelOnValidation<'a> {
        grant: OperationGrant,
        cancel: &'a AtomicBool,
    }
    impl GrantAuthority for CancelOnValidation<'_> {
        fn adapter_id(&self) -> &str {
            "fixture"
        }
        fn authenticate(&self, _: &str) -> Result<OperationGrant, PortError> {
            self.cancel.store(true, Ordering::SeqCst);
            Ok(self.grant.clone())
        }
    }
    let e = evidence(Enforcement::Advise, false, false);
    let d = bare(&e);
    let target = ProtectedBareTarget::open(d.path(), "repo").unwrap();
    let root = tempfile::Builder::new()
        .permissions(std::fs::Permissions::from_mode(0o700))
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap();
    let intents = IntentStore::create(root.path()).unwrap();
    let (session, authority) = session(&e, GrantAction::Merge, "refs/heads/main", "prepared");
    let config = ExecutionConfig { enabled: true };
    let cancel = AtomicBool::new(false);
    let cancelling = CancelOnValidation {
        grant: authority.0.clone(),
        cancel: &cancel,
    };
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
    assert_eq!(
        reconcile(&session, &intents, &target).unwrap(),
        RecoveryState::NotAttempted
    );
    cancel.store(false, Ordering::SeqCst);
    let mut revoked = Authority(authority.0.clone());
    revoked.0.revoked = true;
    context.authority = &revoked;
    assert!(matches!(
        apply(&context, &target),
        Err(ApplyError::Authority)
    ));
    assert_eq!(
        reconcile(&session, &intents, &target).unwrap(),
        RecoveryState::NotAttempted
    );
    assert_eq!(
        common::git(d.path(), &["rev-parse", "refs/heads/main"]),
        e.source.base
    );
    context.authority = &authority;
    apply(&context, &target).unwrap();
    assert!(matches!(
        reconcile(&session, &intents, &target).unwrap(),
        RecoveryState::Applied { .. }
    ));
}
