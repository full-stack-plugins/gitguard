mod common;
use common::*;
use gitguard::{Repository, subject::SubjectRequest};
#[test]
fn merges_only_in_temporary_store_and_keeps_parent_identity() {
    let r = Repo::new("sha1");
    let head = r.commit("b", "branch");
    git(r.path(), &["checkout", "-q", "--detach", &r.base]);
    let base = r.commit("c", "target");
    let before = r.state();
    let repo = Repository::discover(r.path(), "repo").unwrap();
    let merged = repo.preview_merge(&base, &head).unwrap();
    let oid = merged.candidate_oid().unwrap();
    assert_ne!(oid, head);
    assert_ne!(oid, base);
    let reverse = repo.preview_merge(&head, &base).unwrap();
    assert_eq!(merged.digest(), reverse.digest());
    assert_ne!(merged.candidate_oid(), reverse.candidate_oid());
    assert_eq!(before, r.state());
    let tree = git(r.path(), &["rev-parse", "HEAD^{tree}"]);
    assert!(
        repo.resolve_subject(SubjectRequest::TreePreview(tree))
            .unwrap()
            .candidate_oid()
            .is_none()
    );
}
#[test]
fn conflicting_merge_does_not_modify_source() {
    let r = Repo::new("sha1");
    let head = r.commit("a", "head\n");
    git(r.path(), &["checkout", "-q", "--detach", &r.base]);
    let base = r.commit("a", "base changed\n");
    let before = r.state();
    let repo = Repository::discover(r.path(), "repo").unwrap();
    assert!(repo.preview_merge(&base, &head).is_err());
    assert_eq!(before, r.state());
}
