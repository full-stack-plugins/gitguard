mod common;
use common::*;
use gitguard::{
    Repository, candidate::CandidateRequest, scope::TaskScope, subject::SubjectRequest,
};
#[test]
fn exact_queue_candidate_base_group_and_member_order_bind_reuse() {
    let r = Repo::new("sha256");
    let head = r.commit("a", "next");
    let repo = Repository::discover(r.path(), "repo").unwrap();
    let subject = repo
        .resolve_subject(SubjectRequest::Commit(head.clone()))
        .unwrap();
    let scope = TaskScope::advisory(
        "t",
        vec!["R".into()],
        vec![b"a".to_vec()],
        &"a".repeat(64),
        None,
    )
    .unwrap();
    let req = CandidateRequest {
        worktree_id: "worktree-1".into(),
        base_oid: r.base.clone(),
        merge_group_id: Some("group".into()),
        members: vec![r.base.clone(), head.clone()],
    };
    let a = repo.prepare_candidate(&subject, &scope, &req).unwrap();
    assert!(a.advisory());
    let mut reordered = req.clone();
    reordered.members.reverse();
    let b = repo
        .prepare_candidate(&subject, &scope, &reordered)
        .unwrap();
    assert_ne!(a.binding_digest(), b.binding_digest());
    let mut advanced = req.clone();
    advanced.base_oid = head;
    assert_ne!(
        a.binding_digest(),
        repo.prepare_candidate(&subject, &scope, &advanced)
            .unwrap()
            .binding_digest()
    );
    std::fs::write(r.path().join("a"), "dirty").unwrap();
    assert!(repo.verify_source(&subject).is_err());
}
