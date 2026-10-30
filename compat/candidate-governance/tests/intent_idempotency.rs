mod common;
use common::evidence::*;
use gitguard::execution::{grant::*, intent::*};
use guardengine::Enforcement;
use std::collections::BTreeSet;
fn expected(e: &Evidence, actor: &str) -> GrantExpectation {
    GrantExpectation::freeze(
        &e.repo,
        &e.candidate,
        GrantRequest {
            actor: actor.into(),
            action: GrantAction::Merge,
            target: "refs/heads/main".into(),
            expected_target_oid: Some(e.source.base.clone()),
            operation_id: "op-1".into(),
        },
        GrantPolicy {
            authority_adapter: "fixture".into(),
            clock_adapter: "fixture".into(),
            allowed_issuers: BTreeSet::from(["fixture".into()]),
            max_lifetime_seconds: 60,
        },
    )
    .unwrap()
}
#[test]
fn durable_request_replay_conflict_and_claim_never_authorize_git() {
    let e = evidence(Enforcement::Advise, false, false);
    let before = e.source.state();
    let expected = expected(&e, "actor");
    let root = tempfile::Builder::new()
        .prefix(".intent-")
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap();
    let store = IntentStore::create(root.path()).unwrap();
    let receipt = store.prepare(&expected).unwrap();
    assert_eq!(receipt.state(), IntentState::Prepared);
    assert_eq!(store.prepare(&expected).unwrap(), receipt);
    let changed = self::expected(&e, "different");
    assert_eq!(store.prepare(&changed), Err(IntentError::Conflict));
    let reservation = store.claim(&receipt).unwrap();
    assert_eq!(reservation.receipt().state(), IntentState::AttemptRecorded);
    assert!(store.claim(&receipt).is_err());
    drop(store);
    let reopened = IntentStore::open(root.path()).unwrap();
    assert_eq!(
        reopened.prepare(&expected).unwrap().state(),
        IntentState::AttemptRecorded
    );
    assert!(reopened.claim(&receipt).is_err());
    let uncertain = reopened.mark_uncertain(reservation).unwrap();
    assert_eq!(uncertain.state(), IntentState::RecoveryRequired);
    assert_eq!(reopened.prepare(&expected).unwrap(), uncertain);
    assert!(reopened.claim(&uncertain).is_err());
    assert_eq!(e.source.state(), before);
}
