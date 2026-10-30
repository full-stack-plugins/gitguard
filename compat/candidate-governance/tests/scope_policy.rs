use gitguard::scope::TaskScope;
#[test]
fn rejects_traversal_duplicate_requirements_and_missing_policy_digest() {
    for paths in [
        vec![b"../secret".to_vec()],
        vec![b"/absolute".to_vec()],
        vec![b"a/../b".to_vec()],
    ] {
        assert!(
            TaskScope::advisory("task", vec!["R".into()], paths, &"a".repeat(64), None).is_err()
        );
    }
    assert!(
        TaskScope::advisory(
            "t",
            vec!["R".into(), "R".into()],
            vec![],
            &"a".repeat(64),
            None
        )
        .is_err()
    );
    assert!(TaskScope::advisory("t", vec!["R".into()], vec![], "accepted", None).is_err());
}
#[test]
fn scope_nullable_baseline_is_explicit() {
    let scope = TaskScope::advisory(
        "task",
        vec!["R".into()],
        vec![b"a".to_vec()],
        &"a".repeat(64),
        None,
    )
    .unwrap();
    let mut value = serde_json::to_value(scope).unwrap();
    value.as_object_mut().unwrap().remove("baseline_digest");
    assert!(serde_json::from_value::<TaskScope>(value).is_err());
}
#[test]
fn candidate_policy_deletion_cannot_remove_frozen_obligations() {
    use std::process::Command;
    let dir = tempfile::tempdir().unwrap();
    let run = |args: &[&str]| {
        let out = Command::new("/usr/bin/git")
            .arg("-C")
            .arg(dir.path())
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "Fixture")
            .env("GIT_AUTHOR_EMAIL", "f@example.invalid")
            .env("GIT_COMMITTER_NAME", "Fixture")
            .env("GIT_COMMITTER_EMAIL", "f@example.invalid")
            .output()
            .unwrap();
        assert!(out.status.success());
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    };
    run(&["init", "-q"]);
    std::fs::write(dir.path().join("policy.md"), "Required R1").unwrap();
    run(&["add", "."]);
    run(&["commit", "-qm", "base"]);
    let base = run(&["rev-parse", "HEAD"]);
    let scope = TaskScope::advisory(
        "task",
        vec!["R1".into()],
        vec![b"src".to_vec()],
        &"a".repeat(64),
        None,
    )
    .unwrap();
    run(&["rm", "-q", "policy.md"]);
    run(&["commit", "-qm", "delete policy"]);
    let head = run(&["rev-parse", "HEAD"]);
    let repo = gitguard::Repository::discover(dir.path(), "local").unwrap();
    let changes = repo.scan_scope(&base, &head, &scope).unwrap();
    assert_eq!(scope.requirement_ids(), ["R1"]);
    assert_eq!(changes.violations, [b"policy.md".to_vec()]);
}
#[test]
fn digest_only_baseline_cannot_claim_immutable_source_revision() {
    assert!(
        TaskScope::advisory(
            "task",
            vec!["R1".into()],
            vec![b"src".to_vec()],
            &"a".repeat(64),
            Some("b".repeat(64))
        )
        .is_err()
    );
}
