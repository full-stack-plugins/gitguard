mod common;
use common::*;
#[test]
fn discovers_formats_common_dir_and_rejects_bad_roots() {
    for format in ["sha1", "sha256"] {
        let r = Repo::new(format);
        let repo = gitguard::Repository::discover(r.path(), "local-repo").unwrap();
        assert_eq!(repo.object_format(), format);
        assert_eq!(repo.root(), r.path());
        assert_eq!(repo.commit(&r.base).unwrap(), r.base);
        assert!(repo.commit("HEAD").is_err());
        assert!(repo.commit(&"0".repeat(r.base.len())).is_err());
        let linked = r.path().with_extension("linked");
        git(
            r.path(),
            &[
                "worktree",
                "add",
                "-q",
                "--detach",
                linked.to_str().unwrap(),
            ],
        );
        let w = gitguard::Repository::discover(&linked, "local-repo").unwrap();
        assert_eq!(repo.common_dir(), w.common_dir());
        std::fs::remove_dir_all(linked).unwrap();
    }
    let d = tempfile::tempdir().unwrap();
    assert!(gitguard::Repository::discover(d.path(), "local").is_err());
}
#[test]
fn malicious_config_and_alternates_cannot_execute_or_leak() {
    let r = Repo::new("sha1");
    let marker = r.path().join("PWNED");
    git(
        r.path(),
        &[
            "config",
            "core.fsmonitor",
            &format!("touch {}", marker.display()),
        ],
    );
    git(
        r.path(),
        &[
            "config",
            "diff.external",
            &format!("touch {}", marker.display()),
        ],
    );
    git(
        r.path(),
        &["config", "credential.helper", "!echo TOKEN_SUPER_SECRET"],
    );
    let before = r.state();
    let repo = gitguard::Repository::discover(r.path(), "local").unwrap();
    assert_eq!(repo.commit(&r.base).unwrap(), r.base);
    assert!(!marker.exists());
    assert_eq!(before, r.state());
    std::fs::write(
        r.path().join(".git/objects/info/alternates"),
        "/TOKEN_SUPER_SECRET",
    )
    .unwrap();
    let e = gitguard::Repository::discover(r.path(), "local")
        .err()
        .unwrap();
    assert!(!format!("{e:?}").contains("TOKEN_SUPER_SECRET"));
}
