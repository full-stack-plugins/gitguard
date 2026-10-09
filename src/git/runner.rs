//! Only crate-owned argv reaches this runner; source repository config is never loaded.
use crate::Result;
use std::{
    os::unix::process::CommandExt,
    path::Path,
    process::{Command, Stdio},
    time::Duration,
};
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub output_bytes: usize,
    pub timeout: Duration,
    pub storage_bytes: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            output_bytes: 8 * 1024 * 1024,
            timeout: Duration::from_secs(10),
            storage_bytes: 64 * 1024 * 1024,
        }
    }
}
pub(crate) fn run(dir: &Path, args: &[&str], limits: Limits) -> Result<Vec<u8>> {
    let mut c = Command::new("/usr/bin/git");
    c.current_dir(dir)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", dir)
        .env("LC_ALL", "C")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_CONFIG_COUNT", "0")
        .env("GIT_AUTHOR_DATE", "2000-01-01T00:00:00Z")
        .env("GIT_COMMITTER_DATE", "2000-01-01T00:00:00Z");
    c.arg("--no-replace-objects")
        .arg("--git-dir")
        .arg(dir)
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "protocol.allow=never",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.attributesFile=/dev/null",
            "-c",
            "diff.external=",
            "-c",
            "core.pager=cat",
            "-c",
            "core.commitGraph=false",
            "-c",
            "core.multiPackIndex=false",
        ]);
    c.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    super::process::execute(c, limits)
}

// Git 2.50 made fsck start `git refs verify`. Keep that check but launch
// each fixed builtin from our controller under the unchanged process policy.
fn integrity_commands(version: &str) -> Result<Vec<Vec<&'static str>>> {
    let invalid = || crate::Diagnostic::GitFailed;
    if version.len() > 128 {
        return Err(invalid());
    }
    let raw = version.strip_prefix("git version ").ok_or_else(invalid)?;
    let mut fields = raw.split('.');
    let mut number = || -> Result<u32> {
        let field = fields.next().ok_or_else(invalid)?;
        if field.is_empty() || !field.bytes().all(|b| b.is_ascii_digit()) {
            return Err(invalid());
        }
        field.parse().map_err(|_| invalid())
    };
    let major = number()?;
    let minor = number()?;
    let _patch = number()?;
    // Explicit dot-separated distribution suffix, never a prerelease or text.
    for field in fields {
        if field.is_empty() || !field.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Err(invalid());
        }
    }
    if major < 2 {
        return Err(invalid());
    }
    let mut fsck = vec!["fsck", "--full", "--strict", "--no-reflogs"];
    if (major, minor) >= (2, 50) {
        fsck.push("--no-references");
        Ok(vec![vec!["refs", "verify", "--strict"], fsck])
    } else {
        Ok(vec![fsck])
    }
}
pub(crate) fn verify_integrity(dir: &Path, version: &str, limits: Limits) -> Result<()> {
    verify_integrity_with(version, limits, |args, remaining| run(dir, args, remaining))
}
fn verify_integrity_with(
    version: &str,
    limits: Limits,
    mut execute: impl FnMut(&[&str], Limits) -> Result<Vec<u8>>,
) -> Result<()> {
    let maximum = Limits::default();
    if limits.timeout.is_zero()
        || limits.timeout > maximum.timeout
        || limits.output_bytes == 0
        || limits.output_bytes > maximum.output_bytes
        || limits.storage_bytes > maximum.storage_bytes
    {
        return Err(crate::Diagnostic::LimitExceeded);
    }
    let started = std::time::Instant::now();
    for args in integrity_commands(version)? {
        let timeout = limits
            .timeout
            .checked_sub(started.elapsed())
            .filter(|remaining| !remaining.is_zero())
            .ok_or(crate::Diagnostic::LimitExceeded)?;
        execute(&args, Limits { timeout, ..limits })?;
    }
    Ok(())
}

#[cfg(test)]
mod security_tests {
    use super::*;
    use crate::Diagnostic;
    #[test]
    fn actual_git_cannot_create_a_helper_process() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("helper-ran");
        // Fixed hostile fixture, not an exposed caller argv or executable API.
        let alias = format!("alias.boundary=!printf leaked > {}", marker.display());
        let result = run(dir.path(), &["-c", &alias, "boundary"], Limits::default());
        assert!(result.is_err(), "helper process unexpectedly executed");
        assert!(!marker.exists());
    }
    #[test]
    fn callers_cannot_expand_the_supported_resource_profile() {
        let dir = tempfile::tempdir().unwrap();
        let limits = Limits {
            output_bytes: usize::MAX,
            ..Limits::default()
        };
        assert!(matches!(
            run(dir.path(), &["version"], limits),
            Err(Diagnostic::LimitExceeded)
        ));
    }
}

#[cfg(test)]
mod integrity_compatibility_tests {
    use super::*;
    #[test]
    fn references_helper_transition_preserves_both_checks() {
        for version in ["git version 2.47.3", "git version 2.49.9"] {
            assert_eq!(
                integrity_commands(version).unwrap(),
                vec![vec!["fsck", "--full", "--strict", "--no-reflogs"]]
            );
        }
        for version in [
            "git version 2.50.0",
            "git version 2.55.0",
            "git version 2.55.0.distro1",
            "git version 3.0.0",
        ] {
            assert_eq!(
                integrity_commands(version).unwrap(),
                vec![
                    vec!["refs", "verify", "--strict"],
                    vec![
                        "fsck",
                        "--full",
                        "--strict",
                        "--no-reflogs",
                        "--no-references"
                    ]
                ]
            );
        }
        for version in [
            "",
            "git version unknown",
            "git version 2.50",
            "git version 2.50.x",
            "git version 2.50.0-rc1",
            "git version 2.50.0 extra",
            "git version 2.50.0.",
            "git version 999999999999999999999.0.0",
        ] {
            assert!(integrity_commands(version).is_err(), "{version}");
        }
    }
    #[test]
    fn reference_failure_never_falls_back_or_runs_object_check() {
        let mut calls = 0;
        let result = verify_integrity_with("git version 2.55.0", Limits::default(), |args, _| {
            calls += 1;
            assert_eq!(args, ["refs", "verify", "--strict"]);
            Err(crate::Diagnostic::ProcessPolicyDenied)
        });
        assert_eq!(result, Err(crate::Diagnostic::ProcessPolicyDenied));
        assert_eq!(calls, 1);
        let mut calls = 0;
        assert_eq!(
            verify_integrity_with("git version unknown", Limits::default(), |_, _| {
                calls += 1;
                Ok(vec![])
            }),
            Err(crate::Diagnostic::GitFailed)
        );
        assert_eq!(calls, 0);
    }
    #[test]
    fn checks_share_a_deadline_and_preserve_object_failure() {
        let mut calls = 0;
        let mut prior = Limits::default().timeout;
        let result = verify_integrity_with("git version 2.55.0", Limits::default(), |_, limits| {
            assert!(limits.timeout <= prior);
            prior = limits.timeout;
            calls += 1;
            if calls == 1 {
                Ok(vec![])
            } else {
                Err(crate::Diagnostic::GitFailed)
            }
        });
        assert_eq!(calls, 2);
        assert_eq!(result, Err(crate::Diagnostic::GitFailed));
    }
    #[test]
    fn installed_git_rejects_corrupt_refs_and_objects() {
        let dir = tempfile::tempdir().unwrap();
        run(
            dir.path(),
            &["init", "--bare", "--quiet"],
            Limits::default(),
        )
        .unwrap();
        let raw = run(dir.path(), &["version"], Limits::default()).unwrap();
        let version = std::str::from_utf8(&raw).unwrap().trim();
        verify_integrity(dir.path(), version, Limits::default()).unwrap();
        let bad_ref = dir.path().join("refs/heads/broken");
        std::fs::write(&bad_ref, b"not-an-object-id\n").unwrap();
        assert!(verify_integrity(dir.path(), version, Limits::default()).is_err());
        std::fs::remove_file(bad_ref).unwrap();
        let object_dir = dir.path().join("objects/00");
        std::fs::create_dir(&object_dir).unwrap();
        std::fs::write(
            object_dir.join("00000000000000000000000000000000000000"),
            b"corrupt",
        )
        .unwrap();
        assert!(verify_integrity(dir.path(), version, Limits::default()).is_err());
    }
}
