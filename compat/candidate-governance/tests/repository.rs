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
#[test]
fn native_config_sections_and_quotes_determine_real_object_format() {
    let r = Repo::new("sha1");
    git(r.path(), &["config", "example.objectformat", "invented"]);
    git(
        r.path(),
        &[
            "config",
            "remote.origin.url",
            "ssh://untrusted.invalid/arbitrary",
        ],
    );
    let before = r.state();
    let repo = gitguard::Repository::discover(r.path(), "controller-id").unwrap();
    assert_eq!(repo.object_format(), "sha1");
    assert_eq!(repo.repo_id(), "controller-id");
    assert_eq!(
        repo.git_version(),
        String::from_utf8(
            std::process::Command::new("/usr/bin/git")
                .arg("--version")
                .output()
                .unwrap()
                .stdout
        )
        .unwrap()
        .trim()
    );
    let other = gitguard::Repository::discover(&r.path().join("."), "other-id").unwrap();
    assert_eq!(other.repo_id(), "other-id");
    assert_eq!(repo.common_dir(), other.common_dir());
    assert_eq!(repo.commit(&r.base).unwrap(), r.base);
    assert_eq!(before, r.state());
    let r = Repo::new("sha256");
    let config = r.path().join(".git/config");
    let bytes = std::fs::read_to_string(&config).unwrap().replace(
        "objectformat = sha256",
        "objectformat = \"sha256\" # actual native value",
    );
    std::fs::write(&config, bytes).unwrap();
    assert_eq!(
        git(r.path(), &["rev-parse", "--show-object-format"]),
        "sha256"
    );
    let before = r.state();
    let repo = gitguard::Repository::discover(r.path(), "controller-id").unwrap();
    assert_eq!(repo.object_format(), "sha256");
    assert_eq!(repo.commit(&r.base).unwrap(), r.base);
    assert_eq!(before, r.state());
}
#[test]
fn unsupported_repository_version_extensions_and_includes_fail_closed() {
    for suffix in [
        "\n[core]\nrepositoryformatversion = 999\n",
        "\n[extensions]\nfutureMeaning = true\n",
        "\n[include]\npath = /TOKEN_SECRET_CONFIG\n",
    ] {
        let r = Repo::new("sha1");
        let config = r.path().join(".git/config");
        let mut bytes = std::fs::read_to_string(&config).unwrap();
        bytes.push_str(suffix);
        std::fs::write(config, bytes).unwrap();
        let before = r.state();
        let error = gitguard::Repository::discover(r.path(), "controller-id")
            .err()
            .expect("unsupported config must reject");
        assert!(!format!("{error:?}").contains("TOKEN_SECRET_CONFIG"));
        assert_eq!(before, r.state());
    }
}
