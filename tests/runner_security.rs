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
