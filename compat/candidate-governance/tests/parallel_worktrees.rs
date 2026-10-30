mod common;
use common::*;
use gitguard::{Repository, scope::TaskScope, worktree::Registry};
use std::{sync::Arc, time::Duration};
#[test]
fn cancellation_directory_reuse_and_late_completion_are_isolated() {
    let r = Repo::new("sha1");
    let linked = tempfile::tempdir().unwrap();
    let path = linked.path().join("linked");
    git(
        r.path(),
        &["worktree", "add", "-q", "--detach", path.to_str().unwrap()],
    );
    let a = Repository::discover(r.path(), "repo").unwrap();
    let b = Repository::discover(&path, "repo").unwrap();
    let registry = Arc::new(Registry::new().unwrap());
    let scope = |t: &str, id: &str| {
        TaskScope::advisory(
            t,
            vec![id.into()],
            vec![b"a".to_vec()],
            &"a".repeat(64),
            None,
        )
        .unwrap()
    };
    let aa = registry
        .register(&a, &scope("ta", "RA"), Duration::from_secs(60))
        .unwrap();
    let bb = registry
        .register(&b, &scope("tb", "RB"), Duration::from_secs(60))
        .unwrap();
    let before = r.state();
    let old = aa.clone();
    let reg = registry.clone();
    let worker = std::thread::spawn(move || reg.cancel(&old));
    registry.complete(&bb, "B output".into()).unwrap();
    worker.join().unwrap().unwrap();
    let replacement = registry
        .register(&a, &scope("tc", "RC"), Duration::from_secs(60))
        .unwrap();
    assert_ne!(replacement.id(), aa.id());
    assert!(registry.complete(&aa, "late A output".into()).is_err());
    assert_eq!(registry.output(&bb).unwrap(), Some("B output".into()));
    assert!(registry.output(&replacement).unwrap().is_none());
    assert_eq!(before, r.state());
}
