mod common;
use common::*;
use gitguard::{Repository, scope::TaskScope};
#[test]
fn rename_both_ends_binary_paths_and_mode_changes_are_scanned() {
    use std::os::unix::{ffi::OsStringExt, fs::symlink};
    let r = Repo::new("sha1");
    std::fs::create_dir(r.path().join("allowed")).unwrap();
    git(r.path(), &["mv", "a", "allowed/a"]);
    symlink("/outside", r.path().join("allowed/link")).unwrap();
    let binary = std::ffi::OsString::from_vec(b"allowed/\xff".to_vec());
    std::fs::write(r.path().join(binary), "binary path").unwrap();
    git(r.path(), &["add", "."]);
    git(r.path(), &["commit", "-qm", "rename"]);
    let head = git(r.path(), &["rev-parse", "HEAD"]);
    let repo = Repository::discover(r.path(), "local").unwrap();
    let scope = TaskScope::advisory(
        "task",
        vec!["R1".into()],
        vec![b"allowed".to_vec()],
        &"a".repeat(64),
        None,
    )
    .unwrap();
    let d = repo.scan_scope(&r.base, &head, &scope).unwrap();
    assert!(d.violations.iter().any(|p| p == b"a"));
    assert!(
        d.changes
            .iter()
            .any(|c| c.paths.iter().any(|p| p == b"allowed/\xff"))
    );
    assert!(d.changes.iter().any(|c| c.new_mode == "120000"));
    assert!(d.changes.iter().any(|c| c.paths.len() == 2));
}
#[test]
fn submodule_pointer_is_missing_content_coverage() {
    let r = Repo::new("sha1");
    git(
        r.path(),
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{},module", r.base),
        ],
    );
    git(r.path(), &["commit", "-qm", "submodule"]);
    let head = git(r.path(), &["rev-parse", "HEAD"]);
    let repo = Repository::discover(r.path(), "local").unwrap();
    let scope = TaskScope::advisory(
        "t",
        vec!["R".into()],
        vec![b"module".to_vec()],
        &"a".repeat(64),
        None,
    )
    .unwrap();
    assert!(
        !repo
            .scan_scope(&r.base, &head, &scope)
            .unwrap()
            .missing_coverage
            .is_empty()
    );
}
