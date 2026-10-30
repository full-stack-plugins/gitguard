mod common;
use common::evidence::*;
use gitguard::execution::{
    ExecutionConfig,
    apply::{ApplyContext, ApplyError},
    grant::*,
    intent::IntentStore,
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

use gitguard::execution::worktree_apply::{ProtectedWorktreeRoot, apply_worktree};
#[test]
fn creates_exact_detached_worktree_without_changing_source_and_refuses_replay() {
    let e = evidence(Enforcement::Advise, false, false);
    let before = e.source.state();
    let parent = tempfile::Builder::new()
        .permissions(std::fs::Permissions::from_mode(0o700))
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap();
    let target = ProtectedWorktreeRoot::open(parent.path(), "repo").unwrap();
    let root = tempfile::Builder::new()
        .permissions(std::fs::Permissions::from_mode(0o700))
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap();
    let intents = IntentStore::create(root.path()).unwrap();
    let (session, authority) = session(&e, GrantAction::CreateWorktree, "task-worktree", "create");
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
    let result = apply_worktree(&context, &target);
    if cfg!(feature = "privileged-execution") {
        result.unwrap();
        let worktree = parent.path().join("task-worktree");
        assert_eq!(
            common::git(&worktree, &["rev-parse", "HEAD"]),
            e.candidate.candidate_oid()
        );
        assert_eq!(common::git(&worktree, &["status", "--porcelain"]), "");
        for file in e
            .repo
            .read_commit_files(e.candidate.candidate_oid())
            .unwrap()
        {
            use std::os::unix::ffi::OsStrExt;
            assert_eq!(
                std::fs::read(worktree.join(std::ffi::OsStr::from_bytes(file.path()))).unwrap(),
                file.contents()
            );
        }
        assert!(apply_worktree(&context, &target).is_err());
    } else {
        assert!(matches!(result, Err(ApplyError::Disabled)));
        assert!(!parent.path().join("task-worktree").exists());
    }
    assert_eq!(e.source.state(), before);
}

#[cfg(feature = "privileged-execution")]
#[test]
fn rejects_existing_paths_symlinks_and_replaced_parent_before_claim() {
    let e = evidence(Enforcement::Advise, false, false);
    for case in [
        "file",
        "directory",
        "symlink",
        "replaced-root",
        "wrong-action",
        "nested",
    ] {
        let outer = tempfile::Builder::new()
            .permissions(std::fs::Permissions::from_mode(0o700))
            .tempdir_in(env!("CARGO_MANIFEST_DIR"))
            .unwrap();
        let parent = outer.path().join("parent");
        std::fs::create_dir(&parent).unwrap();
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700)).unwrap();
        let target = ProtectedWorktreeRoot::open(&parent, "repo").unwrap();
        match case {
            "file" => std::fs::write(parent.join("task-worktree"), "keep").unwrap(),
            "directory" => std::fs::create_dir(parent.join("task-worktree")).unwrap(),
            "symlink" => {
                std::os::unix::fs::symlink(outer.path(), parent.join("task-worktree")).unwrap()
            }
            "replaced-root" => {
                std::fs::rename(&parent, outer.path().join("old-parent")).unwrap();
                std::fs::create_dir(&parent).unwrap();
                std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700)).unwrap();
            }
            _ => (),
        }
        let root = tempfile::Builder::new()
            .permissions(std::fs::Permissions::from_mode(0o700))
            .tempdir_in(env!("CARGO_MANIFEST_DIR"))
            .unwrap();
        let intents = IntentStore::create(root.path()).unwrap();
        let before = std::fs::read(root.path().join("intent.json")).unwrap();
        let (session, authority) = session(
            &e,
            if case == "wrong-action" {
                GrantAction::CreateBranch
            } else {
                GrantAction::CreateWorktree
            },
            if case == "nested" {
                "nested/task"
            } else if case == "wrong-action" {
                "refs/heads/task"
            } else {
                "task-worktree"
            },
            "create",
        );
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
        assert!(apply_worktree(&context, &target).is_err(), "{case}");
        assert_eq!(
            std::fs::read(root.path().join("intent.json")).unwrap(),
            before,
            "{case}"
        );
        if case == "file" {
            assert_eq!(
                std::fs::read(parent.join("task-worktree")).unwrap(),
                b"keep"
            );
        }
    }
}

#[cfg(feature = "privileged-execution")]
#[test]
fn independent_worktree_publishers_have_one_winner_and_keep_unknown_staging() {
    use gitguard::execution::reconcile::RecoveryState;
    use std::sync::{
        Arc, Barrier,
        atomic::{AtomicUsize, Ordering},
    };
    struct Rendezvous {
        grant: OperationGrant,
        count: AtomicUsize,
        barrier: Arc<Barrier>,
    }
    impl GrantAuthority for Rendezvous {
        fn adapter_id(&self) -> &str {
            "fixture"
        }
        fn authenticate(&self, _: &str) -> Result<OperationGrant, PortError> {
            if self.count.fetch_add(1, Ordering::SeqCst) == 2 {
                self.barrier.wait();
            }
            Ok(self.grant.clone())
        }
    }
    let e = evidence(Enforcement::Advise, false, false);
    let parent = tempfile::Builder::new()
        .permissions(std::fs::Permissions::from_mode(0o700))
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap();
    let target = ProtectedWorktreeRoot::open(parent.path(), "repo").unwrap();
    let roots: Vec<_> = (0..2)
        .map(|_| {
            tempfile::Builder::new()
                .permissions(std::fs::Permissions::from_mode(0o700))
                .tempdir_in(env!("CARGO_MANIFEST_DIR"))
                .unwrap()
        })
        .collect();
    let stores: Vec<_> = roots
        .iter()
        .map(|root| IntentStore::create(root.path()).unwrap())
        .collect();
    let barrier = Arc::new(Barrier::new(2));
    let sessions: Vec<_> = (0..2)
        .map(|i| {
            session(
                &e,
                GrantAction::CreateWorktree,
                "task-worktree",
                &format!("op-{i}"),
            )
        })
        .collect();
    let authorities: Vec<_> = sessions
        .iter()
        .map(|(_, authority)| Rendezvous {
            grant: authority.0.clone(),
            count: AtomicUsize::new(0),
            barrier: barrier.clone(),
        })
        .collect();
    let config = ExecutionConfig { enabled: true };
    let cancel = AtomicBool::new(false);
    let results = std::thread::scope(|scope| {
        let threads: Vec<_> = (0..2)
            .map(|i| {
                let e = &e;
                let target = &target;
                let stores = &stores;
                let sessions = &sessions;
                let authorities = &authorities;
                let config = &config;
                let cancel = &cancel;
                scope.spawn(move || {
                    let context = ApplyContext {
                        config,
                        repository: &e.repo,
                        candidate: &e.candidate,
                        session: &sessions[i].0,
                        authority: &authorities[i],
                        clock: &Clock,
                        grant_reference: "fixture",
                        intents: &stores[i],
                        cancel,
                    };
                    apply_worktree(&context, target)
                })
            })
            .collect();
        threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(ApplyError::RecoveryRequired)))
            .count(),
        1
    );
    assert_eq!(
        common::git(&parent.path().join("task-worktree"), &["rev-parse", "HEAD"]),
        e.candidate.candidate_oid()
    );
    for i in 0..2 {
        let state = target.reconcile(&sessions[i].0, &stores[i]).unwrap();
        if results[i].is_ok() {
            assert!(matches!(state, RecoveryState::Applied { .. }));
        } else {
            assert_eq!(state, RecoveryState::RecoveryRequired);
        }
    }
    assert!(std::fs::read_dir(parent.path()).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .as_encoded_bytes()
            .starts_with(b".gitguard-staging-")
    }));
}

#[cfg(feature = "privileged-execution")]
#[test]
fn prepared_worktree_is_bound_to_parent_and_late_revocation_never_publishes() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Canceller<'a> {
        grant: OperationGrant,
        cancel: &'a AtomicBool,
    }
    impl GrantAuthority for Canceller<'_> {
        fn adapter_id(&self) -> &str {
            "fixture"
        }
        fn authenticate(&self, _: &str) -> Result<OperationGrant, PortError> {
            self.cancel.store(true, Ordering::SeqCst);
            Ok(self.grant.clone())
        }
    }
    struct RevokeAfterBuild {
        grant: OperationGrant,
        count: AtomicUsize,
    }
    impl GrantAuthority for RevokeAfterBuild {
        fn adapter_id(&self) -> &str {
            "fixture"
        }
        fn authenticate(&self, _: &str) -> Result<OperationGrant, PortError> {
            let mut grant = self.grant.clone();
            grant.revoked = self.count.fetch_add(1, Ordering::SeqCst) >= 2;
            Ok(grant)
        }
    }
    let e = evidence(Enforcement::Advise, false, false);
    let parent = tempfile::Builder::new()
        .permissions(std::fs::Permissions::from_mode(0o700))
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap();
    let other = tempfile::Builder::new()
        .permissions(std::fs::Permissions::from_mode(0o700))
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap();
    let target = ProtectedWorktreeRoot::open(parent.path(), "repo").unwrap();
    let redirected = ProtectedWorktreeRoot::open(other.path(), "repo").unwrap();
    let root = tempfile::Builder::new()
        .permissions(std::fs::Permissions::from_mode(0o700))
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap();
    let intents = IntentStore::create(root.path()).unwrap();
    let (session, authority) = session(&e, GrantAction::CreateWorktree, "task-worktree", "create");
    let config = ExecutionConfig { enabled: true };
    let cancel = AtomicBool::new(false);
    let cancelling = Canceller {
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
        apply_worktree(&context, &target),
        Err(ApplyError::Cancelled)
    ));
    cancel.store(false, Ordering::SeqCst);
    context.authority = &authority;
    assert!(matches!(
        apply_worktree(&context, &redirected),
        Err(ApplyError::Intent)
    ));
    assert_eq!(std::fs::read_dir(other.path()).unwrap().count(), 0);
    let revoking = RevokeAfterBuild {
        grant: authority.0.clone(),
        count: AtomicUsize::new(0),
    };
    context.authority = &revoking;
    assert!(matches!(
        apply_worktree(&context, &target),
        Err(ApplyError::RecoveryRequired)
    ));
    assert!(!parent.path().join("task-worktree").exists());
    assert_eq!(
        target.reconcile(&session, &intents).unwrap(),
        gitguard::execution::reconcile::RecoveryState::RecoveryRequired
    );
    context.authority = &authority;
    assert!(apply_worktree(&context, &target).is_err());
}
