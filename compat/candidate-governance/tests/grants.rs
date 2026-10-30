mod common;
use common::evidence::*;
use gitguard::execution::grant::*;
use guardengine::Enforcement;
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicI64, Ordering};
struct Clock(AtomicI64);
impl GrantClock for Clock {
    fn adapter_id(&self) -> &str {
        "fixture-clock"
    }
    fn now(&self) -> Result<i64, PortError> {
        Ok(self.0.load(Ordering::SeqCst))
    }
}
struct Issuer(OperationGrant);
impl GrantAuthority for Issuer {
    fn adapter_id(&self) -> &str {
        "fixture-authority"
    }
    fn authenticate(&self, reference: &str) -> Result<OperationGrant, PortError> {
        if reference == "issuer:grant" {
            Ok(self.0.clone())
        } else {
            Err(PortError::Unavailable)
        }
    }
}
fn request() -> GrantRequest {
    GrantRequest {
        actor: "controller-actor".into(),
        action: GrantAction::Merge,
        target: "refs/heads/main".into(),
        expected_target_oid: Some("b".repeat(40)),
        operation_id: "op-1".into(),
    }
}
fn policy() -> GrantPolicy {
    GrantPolicy {
        authority_adapter: "fixture-authority".into(),
        clock_adapter: "fixture-clock".into(),
        allowed_issuers: BTreeSet::from(["fixture-issuer".into()]),
        max_lifetime_seconds: 60,
    }
}
fn record(e: &Evidence, request: &GrantRequest) -> OperationGrant {
    OperationGrant {
        api_version: "gitguard.operation-grant/v1alpha1".into(),
        issuer: "fixture-issuer".into(),
        actor: request.actor.clone(),
        action: request.action,
        repo_id: e.candidate.repo_id().into(),
        candidate_oid: e.candidate.candidate_oid().into(),
        binding_digest: e.candidate.binding_digest(),
        target: request.target.clone(),
        expected_target_oid: request.expected_target_oid.clone(),
        operation_id: request.operation_id.clone(),
        not_before: 10,
        expires_at: 60,
        revoked: false,
    }
}
#[test]
fn authenticated_exact_grant_is_read_only_and_each_field_drift_fails() {
    let e = evidence(Enforcement::Advise, false, false);
    common::git(e.source.path(), &["branch", "main", &e.source.base]);
    let before = e.source.state();
    let mut req = request();
    req.expected_target_oid = Some(e.source.base.clone());
    let valid = record(&e, &req);
    let expected = GrantExpectation::freeze(&e.repo, &e.candidate, req, policy()).unwrap();
    let session = GrantSession::new(expected);
    let clock = Clock(AtomicI64::new(20));
    let observation = session
        .validate("issuer:grant", &Issuer(valid.clone()), &clock)
        .unwrap();
    assert_eq!(observation.observed_at(), 20);
    for field in 0..13 {
        let mut changed = valid.clone();
        match field {
            0 => changed.issuer = "forged".into(),
            1 => changed.actor = "other-actor".into(),
            2 => changed.action = GrantAction::CreateWorktree,
            3 => changed.repo_id = "other-repo".into(),
            4 => changed.candidate_oid = e.source.base.clone(),
            5 => changed.binding_digest = "f".repeat(64),
            6 => changed.target = "refs/heads/other".into(),
            7 => changed.expected_target_oid = Some("c".repeat(40)),
            8 => changed.operation_id = "op-2".into(),
            9 => changed.not_before = 21,
            10 => changed.expires_at = 20,
            11 => changed.revoked = true,
            _ => changed.expires_at = 1000,
        }
        assert!(
            session
                .validate("issuer:grant", &Issuer(changed), &clock)
                .is_err(),
            "field {field}"
        );
    }
    assert!(
        session
            .validate("self-declared", &Issuer(valid), &clock)
            .is_err()
    );
    for operation in [
        gitguard::execution::Operation::CreateWorktree,
        gitguard::execution::Operation::CreateBranch,
        gitguard::execution::Operation::Push,
        gitguard::execution::Operation::Merge,
    ] {
        assert!(
            gitguard::execution::request(
                &gitguard::execution::ExecutionConfig { enabled: true },
                operation
            )
            .is_err()
        );
    }
    assert_eq!(before, e.source.state());
}

#[test]
fn clock_and_adapter_failures_cannot_revive_old_observations() {
    let e = evidence(Enforcement::Advise, false, false);
    let req = request();
    let valid = record(&e, &req);
    let session =
        GrantSession::new(GrantExpectation::freeze(&e.repo, &e.candidate, req, policy()).unwrap());
    let clock = Clock(AtomicI64::new(20));
    struct Delayed<'a> {
        grant: OperationGrant,
        clock: &'a Clock,
    }
    impl GrantAuthority for Delayed<'_> {
        fn adapter_id(&self) -> &str {
            "fixture-authority"
        }
        fn authenticate(&self, _: &str) -> Result<OperationGrant, PortError> {
            self.clock.0.store(60, Ordering::SeqCst);
            Ok(self.grant.clone())
        }
    }
    assert!(matches!(
        session.validate(
            "issuer:grant",
            &Delayed {
                grant: valid.clone(),
                clock: &clock
            },
            &clock
        ),
        Err(GrantError::Inactive)
    ));
    clock.0.store(20, Ordering::SeqCst);
    assert!(matches!(
        session.validate("issuer:grant", &Issuer(valid.clone()), &clock),
        Err(GrantError::ClockRollback)
    ));
    struct WrongClock;
    impl GrantClock for WrongClock {
        fn adapter_id(&self) -> &str {
            "candidate-clock"
        }
        fn now(&self) -> Result<i64, PortError> {
            panic!("wrong adapter must never be queried")
        }
    }
    assert!(matches!(
        session.validate("issuer:grant", &Issuer(valid.clone()), &WrongClock),
        Err(GrantError::WrongAdapter)
    ));
    struct DownClock;
    impl GrantClock for DownClock {
        fn adapter_id(&self) -> &str {
            "fixture-clock"
        }
        fn now(&self) -> Result<i64, PortError> {
            Err(PortError::Untrusted)
        }
    }
    assert!(matches!(
        session.validate("issuer:grant", &Issuer(valid.clone()), &DownClock),
        Err(GrantError::ClockUnavailable)
    ));
    struct WrongAuthority;
    impl GrantAuthority for WrongAuthority {
        fn adapter_id(&self) -> &str {
            "candidate-authority"
        }
        fn authenticate(&self, _: &str) -> Result<OperationGrant, PortError> {
            panic!("wrong adapter must never be queried")
        }
    }
    assert!(matches!(
        session.validate("issuer:grant", &WrongAuthority, &clock),
        Err(GrantError::WrongAdapter)
    ));
    let session = GrantSession::new(
        GrantExpectation::freeze(&e.repo, &e.candidate, request(), policy()).unwrap(),
    );
    clock.0.store(-1, Ordering::SeqCst);
    assert!(matches!(
        session.validate("issuer:grant", &Issuer(valid), &clock),
        Err(GrantError::ClockRollback)
    ));
}

#[test]
fn reentrant_and_concurrent_provider_completion_cannot_publish_old_success() {
    let e = evidence(Enforcement::Advise, false, false);
    let req = request();
    let valid = record(&e, &req);
    let session =
        GrantSession::new(GrantExpectation::freeze(&e.repo, &e.candidate, req, policy()).unwrap());
    let clock = Clock(AtomicI64::new(20));
    struct Reentrant<'a> {
        session: &'a GrantSession,
        clock: &'a Clock,
        grant: OperationGrant,
    }
    impl GrantAuthority for Reentrant<'_> {
        fn adapter_id(&self) -> &str {
            "fixture-authority"
        }
        fn authenticate(&self, _: &str) -> Result<OperationGrant, PortError> {
            self.session
                .validate("issuer:grant", &Issuer(self.grant.clone()), self.clock)
                .unwrap();
            Ok(self.grant.clone())
        }
    }
    assert!(matches!(
        session.validate(
            "issuer:grant",
            &Reentrant {
                session: &session,
                clock: &clock,
                grant: valid.clone()
            },
            &clock
        ),
        Err(GrantError::Superseded)
    ));
    use std::sync::{Mutex, mpsc};
    let (began_tx, began_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    struct Waiting {
        began: mpsc::Sender<()>,
        release: Mutex<mpsc::Receiver<()>>,
        grant: OperationGrant,
    }
    impl GrantAuthority for Waiting {
        fn adapter_id(&self) -> &str {
            "fixture-authority"
        }
        fn authenticate(&self, _: &str) -> Result<OperationGrant, PortError> {
            self.began.send(()).unwrap();
            self.release
                .lock()
                .unwrap()
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            Ok(self.grant.clone())
        }
    }
    let waiting = Waiting {
        began: began_tx,
        release: Mutex::new(release_rx),
        grant: valid.clone(),
    };
    std::thread::scope(|scope| {
        let old = scope.spawn(|| session.validate("issuer:grant", &waiting, &clock));
        began_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        clock.0.store(60, Ordering::SeqCst);
        assert!(matches!(
            session.validate("issuer:grant", &Issuer(valid.clone()), &clock),
            Err(GrantError::Inactive)
        ));
        clock.0.store(20, Ordering::SeqCst);
        release_tx.send(()).unwrap();
        assert!(matches!(
            old.join().unwrap(),
            Err(GrantError::ClockRollback)
        ));
    });
}

#[test]
fn full_candidate_scope_cannot_be_relabelled_and_wire_schema_is_strict() {
    let e = evidence(Enforcement::Advise, false, false);
    let req = request();
    let valid = record(&e, &req);
    let clock = Clock(AtomicI64::new(20));
    for change in 0..8 {
        let mut json = serde_json::to_value(&e.candidate).unwrap();
        match change {
            0 => json["task_id"] = "different".into(),
            1 => json["worktree_id"] = "different".into(),
            2 => json["requirement_ids"] = serde_json::json!(["OTHER"]),
            3 => json["allowed_paths"] = serde_json::json!([[98]]),
            4 => json["policy_digest"] = "f".repeat(64).into(),
            5 => {
                json["merge_group_id"] = "queue".into();
                json["members"] = serde_json::json!([e.candidate.candidate_oid()]);
            }
            6 => json["source_snapshot_digest"] = "f".repeat(64).into(),
            _ => json["baseline_digest"] = format!("sha256:{}", "f".repeat(64)).into(),
        }
        let changed = serde_json::from_value(json).unwrap();
        if let Ok(expected) = GrantExpectation::freeze(&e.repo, &changed, request(), policy()) {
            assert!(
                GrantSession::new(expected)
                    .validate("issuer:grant", &Issuer(valid.clone()), &clock)
                    .is_err(),
                "binding dimension {change}"
            );
        }
    }
    OperationGrant::from_json(include_bytes!(
        "../fixtures/schema/operation-grant-valid.json"
    ))
    .unwrap();
    let good = serde_json::to_vec(&valid).unwrap();
    OperationGrant::from_json(&good).unwrap();
    for key in ["approved", "signatureVerified", "canMerge"] {
        let mut json = serde_json::to_value(&valid).unwrap();
        json[key] = true.into();
        assert!(OperationGrant::from_json(&serde_json::to_vec(&json).unwrap()).is_err());
    }
    for field in ["api_version", "action", "target", "expected_target_oid"] {
        let mut json = serde_json::to_value(&valid).unwrap();
        json[field] = "unsupported".into();
        assert!(OperationGrant::from_json(&serde_json::to_vec(&json).unwrap()).is_err());
    }
    let text = String::from_utf8(good).unwrap();
    let duplicate = text.replacen("\"issuer\":", "\"issuer\":\"forged\",\"issuer\":", 1);
    assert!(OperationGrant::from_json(duplicate.as_bytes()).is_err());
    let mut json = serde_json::to_value(&valid).unwrap();
    json.as_object_mut().unwrap().remove("expected_target_oid");
    assert!(OperationGrant::from_json(&serde_json::to_vec(&json).unwrap()).is_err());
    assert!(OperationGrant::from_json(&vec![b' '; 16385]).is_err());
    let mut huge = valid;
    huge.issuer = "x".repeat(16385);
    let session = GrantSession::new(
        GrantExpectation::freeze(&e.repo, &e.candidate, request(), policy()).unwrap(),
    );
    assert!(matches!(
        session.validate("issuer:grant", &Issuer(huge), &clock),
        Err(GrantError::Budget)
    ));
}

#[test]
fn create_actions_require_absence_and_never_imply_merge_or_push() {
    let e = evidence(Enforcement::Advise, false, false);
    let clock = Clock(AtomicI64::new(20));
    for action in [
        GrantAction::CreateWorktree,
        GrantAction::CreateBranch,
        GrantAction::Push,
        GrantAction::Merge,
    ] {
        let mut req = request();
        req.action = action;
        if action == GrantAction::CreateWorktree {
            req.target = "worktrees/task".into();
        }
        if matches!(
            action,
            GrantAction::CreateWorktree | GrantAction::CreateBranch
        ) {
            req.expected_target_oid = None;
        }
        let grant = record(&e, &req);
        let session = GrantSession::new(
            GrantExpectation::freeze(&e.repo, &e.candidate, req, policy()).unwrap(),
        );
        session
            .validate("issuer:grant", &Issuer(grant.clone()), &clock)
            .unwrap();
        let mut stronger = grant;
        stronger.action = if action == GrantAction::Merge {
            GrantAction::Push
        } else {
            GrantAction::Merge
        };
        assert!(
            session
                .validate("issuer:grant", &Issuer(stronger), &clock)
                .is_err()
        );
    }
    for literal in [
        "../escape",
        "/absolute",
        "refs/heads/../main",
        "refs/heads/main.lock",
        "refs/heads/a//b",
        "refs/heads/a@{b}",
        "https://example.invalid/ref",
    ] {
        let mut req = request();
        req.target = literal.into();
        assert!(
            GrantExpectation::freeze(&e.repo, &e.candidate, req, policy()).is_err(),
            "{literal}"
        );
    }
    let mut req = request();
    req.actor = "x".repeat(1024 * 1024);
    assert!(matches!(
        GrantExpectation::freeze(&e.repo, &e.candidate, req, policy()),
        Err(GrantError::Budget)
    ));
}
