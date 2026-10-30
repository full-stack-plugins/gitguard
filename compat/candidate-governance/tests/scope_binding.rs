mod common;
use common::*;
use gitguard::{
    Repository,
    candidate::{CandidateRequest, CandidateSnapshot},
    preflight::preflight,
    scope::TaskScope,
    subject::SubjectRequest,
};

fn observe(
    repo: &Repository,
    head: &str,
    base: &str,
    paths: Vec<Vec<u8>>,
) -> gitguard::preflight::PreflightResult {
    let scope =
        TaskScope::advisory("task", vec!["R".into()], paths, &"a".repeat(64), None).unwrap();
    let request = CandidateRequest {
        worktree_id: "w".into(),
        base_oid: base.into(),
        merge_group_id: None,
        members: vec![],
    };
    preflight(repo, SubjectRequest::Commit(head.into()), &scope, &request).unwrap()
}
#[test]
fn actual_path_scope_changes_binding_even_when_external_policy_digest_is_identical() {
    let r = Repo::new("sha1");
    let head = r.commit("a", "candidate");
    let repo = Repository::discover(r.path(), "repo").unwrap();
    let allowed = observe(&repo, &head, &r.base, vec![b"a".to_vec()]);
    let denied = observe(&repo, &head, &r.base, vec![b"b".to_vec()]);
    assert!(allowed.complete);
    assert!(!denied.complete);
    assert_eq!(
        allowed.candidate.policy_digest(),
        denied.candidate.policy_digest()
    );
    assert_ne!(
        allowed.candidate.binding_digest(),
        denied.candidate.binding_digest()
    );
}
#[test]
fn snapshot_scope_is_canonical_byte_sorted_set_and_old_or_noncanonical_records_are_rejected() {
    let r = Repo::new("sha1");
    let repo = Repository::discover(r.path(), "repo").unwrap();
    let canonical = observe(
        &repo,
        &r.base,
        &r.base,
        vec![b"a".to_vec(), b"b".to_vec(), vec![255]],
    );
    let reordered = observe(
        &repo,
        &r.base,
        &r.base,
        vec![vec![255], b"b".to_vec(), b"a".to_vec(), b"a".to_vec()],
    );
    assert_eq!(
        canonical.candidate.binding_digest(),
        reordered.candidate.binding_digest()
    );
    let value = serde_json::to_value(&reordered.candidate).unwrap();
    assert_eq!(
        value["allowed_paths"],
        serde_json::json!([[97], [98], [255]])
    );
    assert_eq!(value["schema_version"], "gitguard.candidate/v1alpha2");
    for bad in [
        serde_json::json!([[98], [97]]),
        serde_json::json!([[97], [97]]),
        serde_json::json!([[46, 46, 47, 97]]),
    ] {
        let mut mutated = value.clone();
        mutated["allowed_paths"] = bad;
        let snapshot: CandidateSnapshot = serde_json::from_value(mutated).unwrap();
        assert!(snapshot.validate(&repo).is_err());
    }
    let mut missing = value.clone();
    missing.as_object_mut().unwrap().remove("allowed_paths");
    assert!(serde_json::from_value::<CandidateSnapshot>(missing).is_err());
    let mut old = value;
    old["schema_version"] = serde_json::json!("gitguard.candidate/v1alpha1");
    assert!(
        serde_json::from_value::<CandidateSnapshot>(old)
            .unwrap()
            .validate(&repo)
            .is_err()
    );
}
