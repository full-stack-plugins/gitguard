mod common;
use common::*;
use gitguard::{
    Repository,
    candidate::{CandidateRequest, CandidateSnapshot},
    scope::TaskScope,
    subject::SubjectRequest,
};
#[test]
fn strict_snapshot_rejects_forged_types_ids_missing_digests_and_implicit_null() {
    let r = Repo::new("sha1");
    let repo = Repository::discover(r.path(), "repo").unwrap();
    let subject = repo
        .resolve_subject(SubjectRequest::Commit(r.base.clone()))
        .unwrap();
    let scope = TaskScope::advisory(
        "task",
        vec!["A".into(), "B".into()],
        vec![b"a".to_vec()],
        &"a".repeat(64),
        None,
    )
    .unwrap();
    let req = CandidateRequest {
        worktree_id: "w".into(),
        base_oid: r.base.clone(),
        merge_group_id: None,
        members: vec![],
    };
    let snapshot = repo.prepare_candidate(&subject, &scope, &req).unwrap();
    let value = serde_json::to_value(&snapshot).unwrap();
    let valid: CandidateSnapshot = serde_json::from_value(value.clone()).unwrap();
    valid.validate(&repo).unwrap();
    for (key, bad) in [
        (
            "candidate_oid",
            serde_json::json!(git(r.path(), &["rev-parse", "HEAD^{tree}"])),
        ),
        ("requirement_ids", serde_json::json!(["A", "A"])),
        ("requirement_ids", serde_json::json!(["B", "A"])),
        ("source_snapshot_digest", serde_json::json!("f".repeat(64))),
        ("advisory", serde_json::json!(false)),
    ] {
        let mut v = value.clone();
        v[key] = bad;
        assert!(
            serde_json::from_value::<CandidateSnapshot>(v)
                .map(|s| s.validate(&repo).is_err())
                .unwrap_or(true)
        );
    }
    for key in [
        "source_snapshot_digest",
        "baseline_digest",
        "merge_group_id",
    ] {
        let mut v = value.clone();
        v.as_object_mut().unwrap().remove(key);
        assert!(
            serde_json::from_value::<CandidateSnapshot>(v).is_err(),
            "missing {key}"
        );
    }
    let mut v = value.clone();
    v["approved"] = serde_json::json!(true);
    assert!(serde_json::from_value::<CandidateSnapshot>(v).is_err());
    std::fs::write(r.path().join("a"), "later dirty").unwrap();
    let stale = repo.prepare_candidate(&subject, &scope, &req).unwrap();
    assert!(
        !stale.clean(),
        "stale clean observation cannot survive changed source bytes"
    );
}
#[test]
fn deserialized_clean_boolean_does_not_override_observed_dirty_source() {
    let r = Repo::new("sha1");
    std::fs::write(r.path().join("a"), "dirty").unwrap();
    let repo = Repository::discover(r.path(), "repo").unwrap();
    let subject = repo
        .resolve_subject(SubjectRequest::Commit(r.base.clone()))
        .unwrap();
    let scope = TaskScope::advisory(
        "task",
        vec!["A".into()],
        vec![b"a".to_vec()],
        &"a".repeat(64),
        None,
    )
    .unwrap();
    let req = CandidateRequest {
        worktree_id: "w".into(),
        base_oid: r.base.clone(),
        merge_group_id: None,
        members: vec![],
    };
    let snapshot = repo.prepare_candidate(&subject, &scope, &req).unwrap();
    let mut value = serde_json::to_value(snapshot).unwrap();
    value["clean"] = serde_json::json!(true);
    let forged: CandidateSnapshot = serde_json::from_value(value).unwrap();
    assert!(forged.validate(&repo).is_err());
}
#[test]
fn snapshot_matches_reproducible_local_sha1_vector() {
    let r = Repo::new("sha1");
    let repo = Repository::discover(r.path(), "repo").unwrap();
    let subject = repo
        .resolve_subject(SubjectRequest::Commit(r.base.clone()))
        .unwrap();
    let scope = TaskScope::advisory(
        "task",
        vec!["R1".into(), "R2".into()],
        vec![b"a".to_vec()],
        &"a".repeat(64),
        None,
    )
    .unwrap();
    let req = CandidateRequest {
        worktree_id: "worktree-1".into(),
        base_oid: r.base.clone(),
        merge_group_id: None,
        members: vec![],
    };
    let snapshot = repo.prepare_candidate(&subject, &scope, &req).unwrap();
    let vector: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/candidate/local-sha1.json")).unwrap();
    assert_eq!(serde_json::to_value(snapshot).unwrap(), vector);
    serde_json::from_value::<CandidateSnapshot>(vector)
        .unwrap()
        .validate(&repo)
        .unwrap();
}
