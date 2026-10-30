mod common;
use common::*;
use gitguard::{Repository, subject::SubjectRequest};
#[test]
fn immutable_commit_index_worktree_and_tree_are_distinct() {
    let r = Repo::new("sha1");
    let repo = Repository::discover(r.path(), "local").unwrap();
    let frozen = repo
        .resolve_subject(SubjectRequest::Commit(r.base.clone()))
        .unwrap();
    assert!(frozen.clean());
    let original = frozen.digest().to_string();
    std::fs::write(r.path().join("a"), "dirty").unwrap();
    let dirty = repo
        .resolve_subject(SubjectRequest::Commit(r.base.clone()))
        .unwrap();
    assert!(!dirty.clean());
    let wt = repo.resolve_subject(SubjectRequest::Worktree).unwrap();
    assert_ne!(wt.digest(), original);
    assert!(wt.candidate_oid().is_none());
    let idx = repo.resolve_subject(SubjectRequest::Index).unwrap();
    assert_eq!(idx.digest(), original);
    assert!(idx.candidate_oid().is_none());
    let tree = git(r.path(), &["rev-parse", "HEAD^{tree}"]);
    assert!(repo.commit(&tree).is_err());
    assert!(
        repo.resolve_subject(SubjectRequest::TreePreview(tree))
            .unwrap()
            .candidate_oid()
            .is_none()
    );
    assert_eq!(frozen.candidate_oid(), Some(r.base.as_str()));
}
#[test]
fn staged_changes_cannot_be_hidden_by_restoring_worktree_bytes() {
    let r = Repo::new("sha1");
    std::fs::write(r.path().join("a"), "staged change").unwrap();
    git(r.path(), &["add", "a"]);
    std::fs::write(r.path().join("a"), "base\n").unwrap();
    let repo = Repository::discover(r.path(), "local").unwrap();
    assert!(
        !repo
            .resolve_subject(SubjectRequest::Commit(r.base.clone()))
            .unwrap()
            .clean()
    );
}
