//! Explicit local CAS dispatch; no generic writer or remote transport is enabled.
use super::{
    ExecutionConfig,
    grant::{GrantAction, GrantAuthority, GrantClock, GrantSession},
    intent::IntentStore,
    platform_write::ProtectedBareTarget,
};
use crate::{Repository, candidate::CandidateSnapshot};
use std::sync::atomic::{AtomicBool, Ordering};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplyError {
    Disabled,
    Unsupported,
    Cancelled,
    Binding,
    Target,
    Authority,
    Intent,
    RecoveryRequired,
}
pub struct ApplyContext<'a> {
    pub config: &'a ExecutionConfig,
    pub repository: &'a Repository,
    pub candidate: &'a CandidateSnapshot,
    pub session: &'a GrantSession,
    pub authority: &'a dyn GrantAuthority,
    pub clock: &'a dyn GrantClock,
    pub grant_reference: &'a str,
    pub intents: &'a IntentStore,
    pub cancel: &'a AtomicBool,
}
/// Historical acknowledgement of local Git CAS, not current ref state or future permission.
pub struct LocalCasReceipt {
    candidate_oid: String,
    operation_id: String,
    request_digest: String,
}
impl LocalCasReceipt {
    pub fn candidate_oid(&self) -> &str {
        &self.candidate_oid
    }
    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }
    pub fn request_digest(&self) -> &str {
        &self.request_digest
    }
}
pub fn apply(
    context: &ApplyContext<'_>,
    target: &ProtectedBareTarget,
) -> Result<LocalCasReceipt, ApplyError> {
    apply_inner(context, target, || Ok(()))
}
fn apply_inner(
    context: &ApplyContext<'_>,
    target: &ProtectedBareTarget,
    after_cas: impl FnOnce() -> Result<(), ApplyError>,
) -> Result<LocalCasReceipt, ApplyError> {
    if !cfg!(feature = "privileged-execution") || !context.config.enabled {
        return Err(ApplyError::Disabled);
    }
    let expected = context.session.expectation();
    if !matches!(
        expected.request().action,
        GrantAction::CreateBranch | GrantAction::Merge
    ) {
        return Err(ApplyError::Unsupported);
    }
    if context.cancel.load(Ordering::SeqCst) {
        return Err(ApplyError::Cancelled);
    }
    crate::evidence::freshness::admit(context.candidate, 1024 * 1024)
        .map_err(|_| ApplyError::Binding)?;
    context
        .candidate
        .validate(context.repository)
        .map_err(|_| ApplyError::Binding)?;
    if expected.binding_digest() != context.candidate.binding_digest()
        || expected.candidate_oid() != context.candidate.candidate_oid()
    {
        return Err(ApplyError::Binding);
    }
    target
        .preflight(context.repository, expected)
        .map_err(|_| ApplyError::Target)?;
    context
        .session
        .validate(context.grant_reference, context.authority, context.clock)
        .map_err(|_| ApplyError::Authority)?;
    let prepared = context
        .intents
        .prepare_for_platform(expected, target.identity())
        .map_err(|_| ApplyError::Intent)?;
    if context.cancel.load(Ordering::SeqCst) {
        return Err(ApplyError::Cancelled);
    }
    let reservation = context
        .intents
        .claim(&prepared)
        .map_err(|_| ApplyError::Intent)?;
    let result = (|| {
        context
            .session
            .validate(context.grant_reference, context.authority, context.clock)
            .map_err(|_| ApplyError::Authority)?;
        if context.cancel.load(Ordering::SeqCst) {
            return Err(ApplyError::Cancelled);
        }
        target
            .compare_and_swap(expected)
            .map_err(|_| ApplyError::Target)?;
        after_cas()?;
        if context.cancel.load(Ordering::SeqCst) {
            return Err(ApplyError::Cancelled);
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = context.intents.mark_uncertain(reservation);
        return Err(ApplyError::RecoveryRequired);
    }
    Ok(LocalCasReceipt {
        candidate_oid: expected.candidate_oid().into(),
        operation_id: expected.operation_id().into(),
        request_digest: prepared.request_digest().into(),
    })
}
#[cfg(all(test, feature = "privileged-execution"))]
mod tests {
    use super::*;
    use crate::execution::grant::*;
    use crate::{candidate::CandidateRequest, scope::TaskScope, subject::SubjectRequest};
    use std::{collections::BTreeSet, path::Path, process::Command};
    fn git(p: &Path, args: &[&str]) -> String {
        let output = Command::new("/usr/bin/git")
            .arg("-C")
            .arg(p)
            .args(args)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "Fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
            .env("GIT_COMMITTER_NAME", "Fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().into()
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
    struct Authority(OperationGrant);
    impl GrantAuthority for Authority {
        fn adapter_id(&self) -> &str {
            "fixture"
        }
        fn authenticate(&self, _: &str) -> Result<OperationGrant, PortError> {
            Ok(self.0.clone())
        }
    }
    struct Clock;
    impl GrantClock for Clock {
        fn adapter_id(&self) -> &str {
            "fixture"
        }
        fn now(&self) -> Result<i64, PortError> {
            Ok(20)
        }
    }
    #[test]
    fn actual_cas_then_delivery_failure_keeps_uncertainty_and_does_not_undo() {
        let source = tempfile::tempdir().unwrap();
        git(source.path(), &["init", "-q"]);
        std::fs::write(source.path().join("a"), "base").unwrap();
        git(source.path(), &["add", "a"]);
        git(source.path(), &["commit", "-qm", "base"]);
        let base = git(source.path(), &["rev-parse", "HEAD"]);
        std::fs::write(source.path().join("a"), "new").unwrap();
        git(source.path(), &["commit", "-qam", "next"]);
        let head = git(source.path(), &["rev-parse", "HEAD"]);
        let repo = Repository::discover(source.path(), "repo").unwrap();
        let subject = repo
            .resolve_subject(SubjectRequest::Commit(head.clone()))
            .unwrap();
        let candidate = repo
            .prepare_candidate(
                &subject,
                &TaskScope::advisory(
                    "task",
                    vec!["R".into()],
                    vec![b"a".to_vec()],
                    &"a".repeat(64),
                    None,
                )
                .unwrap(),
                &CandidateRequest {
                    worktree_id: "w".into(),
                    base_oid: base.clone(),
                    merge_group_id: None,
                    members: vec![],
                },
            )
            .unwrap();
        let request = GrantRequest {
            actor: "fixture".into(),
            action: GrantAction::Merge,
            target: "refs/heads/main".into(),
            expected_target_oid: Some(base.clone()),
            operation_id: "op".into(),
        };
        let authority = Authority(OperationGrant {
            api_version: "gitguard.operation-grant/v1alpha1".into(),
            issuer: "fixture".into(),
            actor: request.actor.clone(),
            action: request.action,
            repo_id: "repo".into(),
            candidate_oid: head.clone(),
            binding_digest: candidate.binding_digest(),
            target: request.target.clone(),
            expected_target_oid: request.expected_target_oid.clone(),
            operation_id: request.operation_id.clone(),
            not_before: 0,
            expires_at: 100,
            revoked: false,
        });
        let session = GrantSession::new(
            GrantExpectation::freeze(
                &repo,
                &candidate,
                request,
                GrantPolicy {
                    authority_adapter: "fixture".into(),
                    clock_adapter: "fixture".into(),
                    allowed_issuers: BTreeSet::from(["fixture".into()]),
                    max_lifetime_seconds: 120,
                },
            )
            .unwrap(),
        );
        let bare = tempfile::tempdir().unwrap();
        git(bare.path(), &["init", "--bare", "-q"]);
        copy(
            &source.path().join(".git/objects"),
            &bare.path().join("objects"),
        );
        git(bare.path(), &["update-ref", "refs/heads/main", &base]);
        let target = ProtectedBareTarget::open(bare.path(), "repo").unwrap();
        let root = tempfile::Builder::new()
            .prefix(".apply-unit-")
            .tempdir_in(env!("CARGO_MANIFEST_DIR"))
            .unwrap();
        let intents = IntentStore::create(root.path()).unwrap();
        let config = ExecutionConfig { enabled: true };
        let cancel = AtomicBool::new(false);
        let context = ApplyContext {
            config: &config,
            repository: &repo,
            candidate: &candidate,
            session: &session,
            authority: &authority,
            clock: &Clock,
            grant_reference: "fixture",
            intents: &intents,
            cancel: &cancel,
        };
        assert!(matches!(
            apply_inner(&context, &target, || Err(ApplyError::RecoveryRequired)),
            Err(ApplyError::RecoveryRequired)
        ));
        assert_eq!(git(bare.path(), &["rev-parse", "refs/heads/main"]), head);
        let j: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.path().join("intent.json")).unwrap())
                .unwrap();
        assert_eq!(j["events"][2]["state"], "recovery_required");
        assert!(apply(&context, &target).is_err());
        assert_eq!(git(bare.path(), &["rev-parse", "refs/heads/main"]), head);
    }
}
