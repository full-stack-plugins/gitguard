mod common;
use common::*;
use gitguard::{Diagnostic, Repository, git::runner::Limits};
use std::time::Duration;
#[test]
fn limits_and_unknown_format_fail_closed_without_source_mutation() {
    let r = Repo::new("sha1");
    let before = r.state();
    assert!(
        Repository::discover_with_limits(
            r.path(),
            "repo",
            Limits {
                output_bytes: 1,
                timeout: Duration::from_millis(100),
                storage_bytes: 1024 * 1024
            }
        )
        .is_err()
    );
    assert!(
        Repository::discover_with_limits(
            r.path(),
            "repo",
            Limits {
                output_bytes: 1024,
                timeout: Duration::ZERO,
                storage_bytes: 1024 * 1024
            }
        )
        .is_err()
    );
    assert!(
        Repository::discover_with_limits(
            r.path(),
            "repo",
            Limits {
                output_bytes: 1024,
                timeout: Duration::from_secs(1),
                storage_bytes: 1
            }
        )
        .is_err()
    );
    assert_eq!(before, r.state());
    std::fs::write(
        r.path().join(".git/config"),
        "[extensions]\nobjectFormat = unknown\n",
    )
    .unwrap();
    assert!(matches!(
        Repository::discover(r.path(), "repo"),
        Err(Diagnostic::UnsupportedFormat)
    ));
}
#[test]
fn symlink_metadata_cannot_redirect_discovery() {
    use std::os::unix::fs::symlink;
    let r = Repo::new("sha1");
    let other = tempfile::tempdir().unwrap();
    symlink(r.path().join(".git"), other.path().join(".git")).unwrap();
    assert!(Repository::discover(other.path(), "repo").is_err());
}

#[test]
fn hostile_git_configuration_and_attributes_never_execute_or_modify_source() {
    use gitguard::scope::TaskScope;
    use std::os::unix::fs::PermissionsExt;
    let r = Repo::new("sha1");
    std::fs::write(
        r.path().join(".gitattributes"),
        "* diff=hostile filter=hostile\n",
    )
    .unwrap();
    let head = r.commit("a", "changed\n");
    let sentinel = tempfile::tempdir().unwrap();
    let marker = sentinel.path().join("executed");
    let script = sentinel.path().join("helper");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\nprintf secret-token > '{}'\nexit 99\n",
            marker.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
    let config = r.path().join(".git/config");
    let mut text = std::fs::read_to_string(&config).unwrap();
    text.push_str(&format!("\n[credential]\nhelper = {}\n[diff]\nexternal = {}\n[diff \"hostile\"]\ntextconv = {}\n[filter \"hostile\"]\nprocess = {}\n[core]\nfsmonitor = {}\nhooksPath = {}\n[remote \"origin\"]\nurl = https://secret-token@example.invalid/private\npromisor = true\n", script.display(),script.display(),script.display(),script.display(),script.display(),sentinel.path().display()));
    std::fs::write(&config, text).unwrap();
    std::fs::copy(&script, sentinel.path().join("post-checkout")).unwrap();
    let before = r.state();
    let repo = Repository::discover(r.path(), "local").unwrap();
    let scope = TaskScope::advisory(
        "t",
        vec!["R".into()],
        vec![b"a".to_vec()],
        &"a".repeat(64),
        None,
    )
    .unwrap();
    assert!(
        !repo
            .scan_scope(&r.base, &head, &scope)
            .unwrap()
            .changes
            .is_empty()
    );
    assert!(repo.commit(&"1".repeat(40)).is_err());
    assert!(!marker.exists());
    assert_eq!(before, r.state());
}

#[test]
fn alternate_and_include_inputs_fail_closed_with_redacted_diagnostics() {
    for name in ["alternates", "http-alternates"] {
        let r = Repo::new("sha1");
        std::fs::write(
            r.path().join(".git/objects/info").join(name),
            "https://secret-token@example.invalid/private\n",
        )
        .unwrap();
        let before = r.state();
        let err = Repository::discover(r.path(), "local").err().unwrap();
        assert_eq!(err, Diagnostic::UnsafeStorage);
        assert!(!format!("{err:?}").contains("secret-token"));
        assert_eq!(before, r.state());
    }
    let r = Repo::new("sha1");
    let path = r.path().join(".git/config");
    let mut config = std::fs::read_to_string(&path).unwrap();
    config.push_str("\n[include]\npath = /secret-token/missing\n");
    std::fs::write(path, config).unwrap();
    assert!(matches!(
        Repository::discover(r.path(), "local"),
        Err(Diagnostic::UnsafeStorage)
    ));
}

#[test]
fn packed_objects_still_validate_without_optional_git_accelerator_helpers() {
    let r = Repo::new("sha1");
    let head = r.commit("a", "second\n");
    git(r.path(), &["gc", "--quiet"]);
    let before = r.state();
    let repo = Repository::discover(r.path(), "local").unwrap();
    assert_eq!(repo.commit(&head).unwrap(), head);
    assert_eq!(before, r.state());
}

#[test]
fn inherited_config_and_helper_environment_is_cleared() {
    let r = Repo::new("sha1");
    let before = r.state();
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "environment_worker", "--nocapture"])
        .env("GG_TEST_ROOT", r.path())
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "include.path")
        .env("GIT_CONFIG_VALUE_0", "/secret-token/missing")
        .env("GIT_OBJECT_DIRECTORY", "/secret-token/objects")
        .env(
            "GIT_ALTERNATE_OBJECT_DIRECTORIES",
            "/secret-token/alternate",
        )
        .env("GIT_EXEC_PATH", "/secret-token/bin")
        .env("GIT_SSH_COMMAND", "/secret-token/helper")
        .env("GIT_EXTERNAL_DIFF", "/secret-token/diff")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!String::from_utf8_lossy(&result.stdout).contains("secret-token"));
    assert_eq!(before, r.state());
}
#[test]
fn environment_worker() {
    if let Some(root) = std::env::var_os("GG_TEST_ROOT") {
        let repo = Repository::discover(std::path::Path::new(&root), "local").unwrap();
        assert!(repo.git_version().starts_with("git version "));
    }
}

#[test]
fn newline_filename_remains_one_nul_delimited_path() {
    let r = Repo::new("sha1");
    let head = r.commit("line\nbreak", "bytes\n");
    let before = r.state();
    let repo = Repository::discover(r.path(), "local").unwrap();
    let scope = gitguard::scope::TaskScope::advisory(
        "t",
        vec!["R".into()],
        vec![b"line\nbreak".to_vec()],
        &"a".repeat(64),
        None,
    )
    .unwrap();
    let changes = repo.scan_scope(&r.base, &head, &scope).unwrap();
    assert_eq!(changes.changes.len(), 1);
    assert_eq!(changes.changes[0].paths, vec![b"line\nbreak".to_vec()]);
    assert!(changes.violations.is_empty());
    assert_eq!(before, r.state());
}
