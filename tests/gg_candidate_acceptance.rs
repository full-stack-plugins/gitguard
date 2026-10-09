mod common;
use common::*;
use gitguard::{
    Repository, candidate::CandidateRequest, preflight::preflight, scope::TaskScope,
    subject::SubjectRequest,
};
#[test]
fn two_requirements_exact_queue_dirty_drift_and_error_without_services() {
    let r = Repo::new("sha1");
    let head = r.commit("a", "candidate");
    let repo = Repository::discover(r.path(), "repo").unwrap();
    let before = r.state();
    let request = CandidateRequest {
        worktree_id: "w".into(),
        base_oid: r.base.clone(),
        merge_group_id: Some("group".into()),
        members: vec![head.clone()],
    };
    let scope = |task: &str, id: &str| {
        TaskScope::advisory(
            task,
            vec![id.into()],
            vec![b"a".to_vec()],
            &"a".repeat(64),
            None,
        )
        .unwrap()
    };
    let a = preflight(
        &repo,
        SubjectRequest::Commit(head.clone()),
        &scope("a", "RA"),
        &request,
    )
    .unwrap();
    let b = preflight(
        &repo,
        SubjectRequest::Commit(head.clone()),
        &scope("b", "RB"),
        &request,
    )
    .unwrap();
    assert!(a.candidate.advisory());
    assert!(a.complete);
    assert_ne!(a.candidate.binding_digest(), b.candidate.binding_digest());
    assert_eq!(a.candidate.candidate_oid(), head);
    assert_eq!(before, r.state());
    std::fs::write(r.path().join("a"), "dirty").unwrap();
    let dirty = preflight(
        &repo,
        SubjectRequest::Commit(head),
        &scope("a", "RA"),
        &request,
    )
    .unwrap();
    assert!(!dirty.candidate.clean());
    assert!(!dirty.complete);
    assert!(
        preflight(
            &repo,
            SubjectRequest::Commit("0".repeat(40)),
            &scope("a", "RA"),
            &request
        )
        .is_err()
    );
    assert!(preflight(&repo, SubjectRequest::Worktree, &scope("a", "RA"), &request).is_err());
}
