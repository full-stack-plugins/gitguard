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
        GrantAction::CreateBranch | GrantAction::Merge | GrantAction::Push
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
    #[cfg(test)]
    crash_checkpoint("before-claim");
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
        if expected.request().action == GrantAction::Push {
            target
                .transfer_candidate(context.repository, expected)
                .map_err(|_| ApplyError::Target)?;
            context
                .session
                .validate(context.grant_reference, context.authority, context.clock)
                .map_err(|_| ApplyError::Authority)?;
            if context.cancel.load(Ordering::SeqCst) {
                return Err(ApplyError::Cancelled);
            }
        }
        #[cfg(test)]
        crash_checkpoint("before-cas");
        target
            .compare_and_swap(expected, prepared.request_digest())
            .map_err(|_| ApplyError::Target)?;
        #[cfg(test)]
        crash_checkpoint("after-cas");
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
    #[cfg(test)]
    crash_checkpoint("before-ack");
    if context.intents.mark_applied(&reservation).is_err() {
        let _ = context.intents.mark_uncertain(reservation);
        return Err(ApplyError::RecoveryRequired);
    }
    #[cfg(test)]
    crash_checkpoint("after-ack");
    Ok(LocalCasReceipt {
        candidate_oid: expected.candidate_oid().into(),
        operation_id: expected.operation_id().into(),
        request_digest: prepared.request_digest().into(),
    })
}
#[cfg(test)]
fn crash_checkpoint(point: &str) {
    if std::env::var("GG_APPLY_TEST_CRASH").as_deref() == Ok(point) {
        // The worker's actual process dies without stack unwinding or TempDir cleanup.
        unsafe {
            libc::kill(libc::getpid(), libc::SIGKILL);
        }
        unreachable!("SIGKILL must terminate the fixture worker");
    }
}
#[cfg(all(test, feature = "privileged-execution"))]
mod tests {
    use super::*;
    use crate::execution::grant::*;
    use crate::{candidate::CandidateRequest, scope::TaskScope, subject::SubjectRequest};
    use std::{collections::BTreeSet, os::unix::fs::PermissionsExt, path::Path, process::Command};
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
    fn fixture_root() -> std::path::PathBuf {
        std::env::var_os("GG_APPLY_TEST_ROOT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| env!("CARGO_MANIFEST_DIR").into())
    }
    #[test]
    fn actual_cas_then_delivery_failure_keeps_uncertainty_and_does_not_undo() {
        let source = tempfile::tempdir_in(fixture_root()).unwrap();
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
        let bare = tempfile::Builder::new()
            .permissions(std::fs::Permissions::from_mode(0o700))
            .tempdir_in(fixture_root())
            .unwrap();
        if std::env::var_os("GG_APPLY_TEST_REFTABLE").is_some() {
            git(
                bare.path(),
                &["init", "--bare", "--ref-format=reftable", "-q"],
            );
        } else {
            git(bare.path(), &["init", "--bare", "-q"]);
        }
        copy(
            &source.path().join(".git/objects"),
            &bare.path().join("objects"),
        );
        git(bare.path(), &["update-ref", "refs/heads/main", &base]);
        let target = if std::env::var_os("GG_APPLY_TEST_REFTABLE").is_some() {
            ProtectedBareTarget::open_reftable_receipts(bare.path(), "repo").unwrap()
        } else {
            ProtectedBareTarget::open(bare.path(), "repo").unwrap()
        };
        let root = tempfile::Builder::new()
            .permissions(std::fs::Permissions::from_mode(0o700))
            .prefix(".apply-unit-")
            .tempdir_in(fixture_root())
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
        if std::env::var_os("GG_APPLY_TEST_ROOT").is_some() {
            std::fs::write(
                fixture_root().join("fixture.json"),
                serde_json::to_vec(&serde_json::json!({
                    "source": source.path(), "target": bare.path(), "intents": root.path(),
                    "base": base, "candidate": candidate,
                }))
                .unwrap(),
            )
            .unwrap();
            if std::env::var("GG_APPLY_TEST_CRASH").as_deref() == Ok("cancel-after-cas") {
                let result = apply_inner(&context, &target, || {
                    context.cancel.store(true, Ordering::SeqCst);
                    Ok(())
                });
                assert!(matches!(result, Err(ApplyError::RecoveryRequired)));
                std::process::exit(82);
            }
            let result = apply(&context, &target).err();
            panic!("worker must crash at the requested checkpoint: {result:?}");
        }
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
        use crate::execution::reconcile::{RecoveryState, reconcile};
        assert_eq!(
            reconcile(&session, &intents, &target).unwrap(),
            RecoveryState::RecoveryRequired
        );
        let tree = git(bare.path(), &["rev-parse", &format!("{head}^{{tree}}")]);
        let later = git(
            bare.path(),
            &["commit-tree", &tree, "-p", &head, "-m", "later"],
        );
        git(bare.path(), &["update-ref", "refs/heads/main", &later]);
        drop(intents);
        let reopened = IntentStore::open(root.path()).unwrap();
        assert_eq!(
            reconcile(&session, &reopened, &target).unwrap(),
            RecoveryState::RecoveryRequired
        );
        assert_eq!(git(bare.path(), &["rev-parse", "refs/heads/main"]), later);
    }
    #[test]
    fn real_process_crash_matrix_preserves_unknown_or_ack_after_target_advances() {
        use crate::execution::reconcile::{RecoveryState, reconcile};
        for point in [
            "before-claim",
            "before-cas",
            "after-cas",
            "before-ack",
            "after-ack",
            "cancel-after-cas",
        ] {
            let fixture = tempfile::Builder::new()
                .permissions(std::fs::Permissions::from_mode(0o700))
                .tempdir_in(env!("CARGO_MANIFEST_DIR"))
                .unwrap();
            let output = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "execution::apply::tests::actual_cas_then_delivery_failure_keeps_uncertainty_and_does_not_undo", "--nocapture"])
                .env("GG_APPLY_TEST_ROOT", fixture.path()).env("GG_APPLY_TEST_CRASH", point)
                .output().unwrap();
            use std::os::unix::process::ExitStatusExt;
            if point == "cancel-after-cas" {
                assert_eq!(output.status.code(), Some(82));
            } else {
                assert_eq!(
                    output.status.signal(),
                    Some(libc::SIGKILL),
                    "{point}: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            let metadata: serde_json::Value = serde_json::from_slice(
                &std::fs::read(fixture.path().join("fixture.json")).unwrap(),
            )
            .unwrap();
            let repo =
                Repository::discover(Path::new(metadata["source"].as_str().unwrap()), "repo")
                    .unwrap();
            let candidate: CandidateSnapshot =
                serde_json::from_value(metadata["candidate"].clone()).unwrap();
            let base = metadata["base"].as_str().unwrap();
            let bare = Path::new(metadata["target"].as_str().unwrap());
            let intents =
                IntentStore::open(Path::new(metadata["intents"].as_str().unwrap())).unwrap();
            let target = ProtectedBareTarget::open(bare, "repo").unwrap();
            let session = GrantSession::new(
                GrantExpectation::freeze(
                    &repo,
                    &candidate,
                    GrantRequest {
                        actor: "fixture".into(),
                        action: GrantAction::Merge,
                        target: "refs/heads/main".into(),
                        expected_target_oid: Some(base.into()),
                        operation_id: "op".into(),
                    },
                    GrantPolicy {
                        authority_adapter: "fixture".into(),
                        clock_adapter: "fixture".into(),
                        allowed_issuers: BTreeSet::from(["fixture".into()]),
                        max_lifetime_seconds: 120,
                    },
                )
                .unwrap(),
            );
            let after_crash = git(bare, &["rev-parse", "refs/heads/main"]);
            assert_eq!(
                after_crash,
                if matches!(point, "before-claim" | "before-cas") {
                    base
                } else {
                    candidate.candidate_oid()
                }
            );
            let initial = reconcile(&session, &intents, &target).unwrap();
            if point == "after-ack" {
                assert!(matches!(initial, RecoveryState::Applied { .. }));
            } else if point == "before-claim" {
                assert_eq!(initial, RecoveryState::NotAttempted);
            } else {
                assert_eq!(initial, RecoveryState::RecoveryRequired);
            }
            let tree = git(
                bare,
                &[
                    "rev-parse",
                    &format!("{}^{{tree}}", candidate.candidate_oid()),
                ],
            );
            let later = git(
                bare,
                &[
                    "commit-tree",
                    &tree,
                    "-p",
                    candidate.candidate_oid(),
                    "-m",
                    "later",
                ],
            );
            git(bare, &["update-ref", "refs/heads/main", &later]);
            assert_eq!(reconcile(&session, &intents, &target).unwrap(), initial);
            println!(
                "{}",
                serde_json::json!({"checkpoint": point, "child_signal": output.status.signal(), "child_exit": output.status.code(),
                "after_crash_oid": after_crash, "advanced_oid": later,
                "recovery": format!("{initial:?}")})
            );
        }
    }
    #[test]
    fn reftable_receipt_resolves_lost_ack_after_target_advances() {
        use crate::execution::reconcile::{RecoveryState, reconcile};
        for point in ["after-cas", "before-ack"] {
            let fixture = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
            let output = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "execution::apply::tests::actual_cas_then_delivery_failure_keeps_uncertainty_and_does_not_undo", "--nocapture"])
                .env("GG_APPLY_TEST_ROOT", fixture.path())
                .env("GG_APPLY_TEST_REFTABLE", "1")
                .env("GG_APPLY_TEST_CRASH", point)
                .output().unwrap();
            use std::os::unix::process::ExitStatusExt;
            assert_eq!(
                output.status.signal(),
                Some(libc::SIGKILL),
                "{point}: {} {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let metadata: serde_json::Value = serde_json::from_slice(
                &std::fs::read(fixture.path().join("fixture.json")).unwrap(),
            )
            .unwrap();
            let repo =
                Repository::discover(Path::new(metadata["source"].as_str().unwrap()), "repo")
                    .unwrap();
            let candidate: CandidateSnapshot =
                serde_json::from_value(metadata["candidate"].clone()).unwrap();
            let bare = Path::new(metadata["target"].as_str().unwrap());
            let journal = Path::new(metadata["intents"].as_str().unwrap());
            let intents = IntentStore::open(journal).unwrap();
            let historical = std::fs::read(journal.join("intent.json")).unwrap();
            let target = ProtectedBareTarget::open_reftable_receipts(bare, "repo").unwrap();
            let session = GrantSession::new(
                GrantExpectation::freeze(
                    &repo,
                    &candidate,
                    GrantRequest {
                        actor: "fixture".into(),
                        action: GrantAction::Merge,
                        target: "refs/heads/main".into(),
                        expected_target_oid: Some(metadata["base"].as_str().unwrap().into()),
                        operation_id: "op".into(),
                    },
                    GrantPolicy {
                        authority_adapter: "fixture".into(),
                        clock_adapter: "fixture".into(),
                        allowed_issuers: BTreeSet::from(["fixture".into()]),
                        max_lifetime_seconds: 120,
                    },
                )
                .unwrap(),
            );
            let before = reconcile(&session, &intents, &target).unwrap();
            assert!(
                matches!(&before, RecoveryState::Applied { candidate_oid, operation_id, .. }
                if candidate_oid == candidate.candidate_oid() && operation_id == "op")
            );
            let tree = git(
                bare,
                &[
                    "rev-parse",
                    &format!("{}^{{tree}}", candidate.candidate_oid()),
                ],
            );
            let later = git(
                bare,
                &[
                    "commit-tree",
                    &tree,
                    "-p",
                    candidate.candidate_oid(),
                    "-m",
                    "later",
                ],
            );
            git(bare, &["update-ref", "refs/heads/main", &later]);
            assert_eq!(reconcile(&session, &intents, &target).unwrap(), before);
            assert_eq!(
                std::fs::read(journal.join("intent.json")).unwrap(),
                historical
            );
            let receipt_ref = git(
                bare,
                &[
                    "for-each-ref",
                    "--format=%(refname)",
                    "refs/gitguard/receipts/",
                ],
            );
            assert!(!receipt_ref.is_empty());
            let original_receipt = git(bare, &["cat-file", "blob", &receipt_ref]);
            let decoded: serde_json::Value = serde_json::from_str(&original_receipt).unwrap();
            // Operation ID, request digest, platform and candidate must all match;
            // valid JSON in a real blob is insufficient proof of this operation.
            for field in [1usize, 2, 4, 5] {
                let mut wrong = decoded.clone();
                wrong[field] = serde_json::Value::String("wrong-binding".into());
                let input = fixture.path().join("wrong-receipt.json");
                std::fs::write(&input, serde_json::to_vec(&wrong).unwrap()).unwrap();
                let blob = git(bare, &["hash-object", "-w", input.to_str().unwrap()]);
                git(bare, &["update-ref", &receipt_ref, &blob]);
                assert_eq!(
                    reconcile(&session, &intents, &target).unwrap(),
                    RecoveryState::RecoveryRequired
                );
            }
            // A valid commit is not a matching receipt, even when it is the candidate.
            git(
                bare,
                &["update-ref", &receipt_ref, candidate.candidate_oid()],
            );
            assert_eq!(
                reconcile(&session, &intents, &target).unwrap(),
                RecoveryState::RecoveryRequired
            );
            git(bare, &["update-ref", "-d", &receipt_ref]);
            assert_eq!(
                reconcile(&session, &intents, &target).unwrap(),
                RecoveryState::RecoveryRequired
            );
            assert_eq!(git(bare, &["rev-parse", "refs/heads/main"]), later);
            let RecoveryState::Applied { request_digest, .. } = &before else {
                unreachable!()
            };
            assert!(
                target
                    .compare_and_swap(session.expectation(), request_digest)
                    .is_err()
            );
            assert!(
                git(
                    bare,
                    &[
                        "for-each-ref",
                        "--format=%(refname)",
                        "refs/gitguard/receipts/"
                    ]
                )
                .is_empty()
            );
            assert_eq!(git(bare, &["rev-parse", "refs/heads/main"]), later);
            assert_eq!(
                std::fs::read(journal.join("intent.json")).unwrap(),
                historical
            );
            println!(
                "{}",
                serde_json::json!({"checkpoint":point, "signal":output.status.signal(),
                "candidate":candidate.candidate_oid(), "advanced":later, "receipt":receipt_ref,
                "recovery":format!("{before:?}"), "missing_or_corrupt":"RecoveryRequired", "journal_unchanged":true})
            );
        }
    }
    #[test]
    fn reftable_receipt_profile_requires_explicit_exact_backend_configuration() {
        let bare = tempfile::Builder::new()
            .permissions(std::fs::Permissions::from_mode(0o700))
            .tempdir_in(env!("CARGO_MANIFEST_DIR"))
            .unwrap();
        git(bare.path(), &["init", "--bare", "-q"]);
        assert!(ProtectedBareTarget::open(bare.path(), "repo").is_ok());
        assert!(ProtectedBareTarget::open_reftable_receipts(bare.path(), "repo").is_err());
        let table = tempfile::Builder::new()
            .permissions(std::fs::Permissions::from_mode(0o700))
            .tempdir_in(env!("CARGO_MANIFEST_DIR"))
            .unwrap();
        git(
            table.path(),
            &["init", "--bare", "--ref-format=reftable", "-q"],
        );
        assert!(ProtectedBareTarget::open(table.path(), "repo").is_err());
        assert!(ProtectedBareTarget::open_reftable_receipts(table.path(), "repo").is_ok());
        git(
            table.path(),
            &["config", "core.hooksPath", "/unused-fixture-path"],
        );
        assert!(ProtectedBareTarget::open_reftable_receipts(table.path(), "repo").is_err());
        git(table.path(), &["config", "--unset", "core.hooksPath"]);
        git(
            table.path(),
            &["config", "extensions.refStorage", "unknown"],
        );
        assert!(ProtectedBareTarget::open_reftable_receipts(table.path(), "repo").is_err());
    }
}
