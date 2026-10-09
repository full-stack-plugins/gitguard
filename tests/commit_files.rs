mod common;
use common::{Repo, git};
use gitguard::{Diagnostic, Repository, git::runner::Limits};
#[test]
fn committed_binary_files_modes_and_raw_paths_are_independent_of_head() {
    for format in ["sha1", "sha256"] {
        let r = Repo::new(format);
        let binary = b"\0\xff\n\r\t";
        std::fs::write(r.path().join("line\nfile"), binary).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            r.path().join("line\nfile"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        git(r.path(), &["add", "."]);
        git(r.path(), &["commit", "-qm", "binary"]);
        let oid = git(r.path(), &["rev-parse", "HEAD"]);
        git(r.path(), &["checkout", "-q", &r.base]);
        std::fs::write(r.path().join("a"), "dirty").unwrap();
        let repo = Repository::discover(r.path(), "local").unwrap();
        let before = r.state();
        let entries = repo.read_commit_files(&oid).unwrap();
        assert_eq!(entries.len(), 2);
        let f = entries.iter().find(|e| e.path() == b"line\nfile").unwrap();
        assert_eq!(f.contents(), binary);
        assert_eq!(f.mode(), "100755");
        assert_eq!(f.object_oid().len(), oid.len());
        assert_eq!(
            entries
                .iter()
                .find(|e| e.path() == b"a")
                .unwrap()
                .contents(),
            b"base\n"
        );
        assert_eq!(before, r.state());
    }
}
#[test]
fn absent_noncommit_and_nonregular_inputs_are_rejected() {
    let r = Repo::new("sha1");
    let repo = Repository::discover(r.path(), "local").unwrap();
    assert!(repo.read_commit_files("HEAD").is_err());
    assert!(repo.read_commit_files(&"1".repeat(40)).is_err());
    let tree = git(r.path(), &["rev-parse", "HEAD^{tree}"]);
    assert!(repo.read_commit_files(&tree).is_err());
    std::os::unix::fs::symlink("a", r.path().join("link")).unwrap();
    git(r.path(), &["add", "."]);
    git(r.path(), &["commit", "-qm", "link"]);
    let oid = git(r.path(), &["rev-parse", "HEAD"]);
    let repo = Repository::discover(r.path(), "local").unwrap();
    assert!(repo.read_commit_files(&oid).is_err());
    git(r.path(), &["rm", "link"]);
    git(
        r.path(),
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{},submodule", r.base),
        ],
    );
    git(r.path(), &["commit", "-qm", "gitlink"]);
    let oid = git(r.path(), &["rev-parse", "HEAD"]);
    let repo = Repository::discover(r.path(), "local").unwrap();
    assert!(repo.read_commit_files(&oid).is_err());
}
#[test]
fn declared_blob_size_is_limited_before_expansion() {
    let r = Repo::new("sha1");
    let oid = r.commit("large", &"x".repeat(1024 * 1024));
    let repo = Repository::discover_with_limits(
        r.path(),
        "local",
        Limits {
            storage_bytes: 16384,
            ..Limits::default()
        },
    )
    .unwrap();
    assert!(matches!(
        repo.read_commit_files(&oid),
        Err(Diagnostic::LimitExceeded)
    ));
}
#[test]
fn altered_object_content_under_expected_oid_is_not_accepted() {
    let r = Repo::new("sha1");
    let first = git(r.path(), &["rev-parse", "HEAD:a"]);
    r.commit("b", "evil\n");
    let second = git(r.path(), &["rev-parse", "HEAD:b"]);
    let object = |id: &str| r.path().join(".git/objects").join(&id[..2]).join(&id[2..]);
    let frozen = Repository::discover(r.path(), "local").unwrap();
    std::fs::remove_file(object(&first)).unwrap();
    std::fs::copy(object(&second), object(&first)).unwrap();
    assert!(Repository::discover(r.path(), "local").is_err());
    let files = frozen.read_commit_files(&r.base).unwrap();
    assert_eq!(files[0].contents(), b"base\n");
}

#[test]
fn one_deadline_covers_many_blob_reads() {
    let r = Repo::new("sha1");
    for n in 0..1000 {
        std::fs::write(r.path().join(format!("f{n:04}")), "same blob").unwrap();
    }
    git(r.path(), &["add", "."]);
    git(r.path(), &["commit", "-qm", "many entries"]);
    let oid = git(r.path(), &["rev-parse", "HEAD"]);
    let repo = Repository::discover_with_limits(
        r.path(),
        "local",
        Limits {
            timeout: std::time::Duration::from_millis(250),
            ..Limits::default()
        },
    )
    .unwrap();
    let started = std::time::Instant::now();
    assert!(matches!(
        repo.read_commit_files(&oid),
        Err(Diagnostic::LimitExceeded)
    ));
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
}
