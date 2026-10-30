#![cfg(target_os = "linux")]
mod common;
use common::evidence::*;
use gitguard::evidence::{
    consume::{ConsumptionContext, consume, consume_bound},
    durable::DurableHistory,
    freshness::ReuseKey,
};
use guardengine::{Enforcement, integration::attempt_store::Target};
#[test]
fn real_consumption_resumes_after_restart_but_changed_full_key_cannot_relabel() {
    let e = evidence(Enforcement::Advise, false, false);
    let p = policy(&e.bundle);
    let key = |dependency: &str| {
        ReuseKey::new(
            &e.repo,
            &e.candidate,
            &p,
            &format!("sha256:{}", dependency.repeat(64)),
            &format!("sha256:{}", "b".repeat(64)),
        )
        .unwrap()
    };
    let original = key("a");
    let changed = key("c");
    let authority = FixtureAuthority::new(p.clone());
    let bound = consume_bound(
        &e.repo,
        &e.candidate,
        &e.bundle,
        &p,
        &authority,
        ConsumptionContext {
            key: &original,
            now: NOW,
            cause: None,
        },
    )
    .unwrap();
    let plain = consume(&e.repo, &e.candidate, &e.bundle, &p, &authority, NOW, None).unwrap();
    let dir = tempfile::Builder::new()
        .prefix(".gg-durable-test-")
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap();
    let store = DurableHistory::create(dir.path()).unwrap();
    store
        .begin(&original, 0, &e.bundle.envelope.run_id)
        .unwrap();
    drop(store);
    let store = DurableHistory::open(dir.path()).unwrap();
    assert!(store.resume(&changed, &e.bundle.envelope.run_id).is_err());
    let ticket = store.resume(&original, &e.bundle.envelope.run_id).unwrap();
    assert!(store.complete(&ticket, &plain).is_err());
    store.complete(&ticket, &bound).unwrap();
    store.publish(&ticket).unwrap();
    drop(store);
    let store = DurableHistory::open(dir.path()).unwrap();
    let target = Target {
        repo_id: "repo".into(),
        task_id: "task".into(),
        requirement_ids: vec!["R".into()],
    };
    assert!(store.current(&target).unwrap().unwrap().eligible);
    assert_eq!(store.history(&target).unwrap().len(), 1);
    let other_dir = tempfile::Builder::new()
        .prefix(".gg-durable-test-")
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap();
    let other = DurableHistory::create(other_dir.path()).unwrap();
    let changed_ticket = other.begin(&changed, 0, &e.bundle.envelope.run_id).unwrap();
    assert!(other.complete(&changed_ticket, &bound).is_err());
    assert!(other.publish(&ticket).is_err());
}
