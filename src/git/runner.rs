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
