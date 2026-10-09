//! Only crate-owned argv reaches this runner; source repository config is never loaded.
use crate::{Diagnostic, Result};
use std::{
    io::Read,
    os::unix::process::CommandExt,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
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
    c.env_clear()
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
        ]);
    c.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let mut child = c.spawn().map_err(|_| Diagnostic::GitFailed)?;
    let pid = child.id() as i32;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let max = limits.output_bytes;
    fn collect(r: impl Read, max: usize) -> std::io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        r.take(max.saturating_add(1) as u64)
            .read_to_end(&mut bytes)?;
        Ok(bytes)
    }
    let out = thread::spawn(move || collect(stdout, max));
    let err = thread::spawn(move || collect(stderr, max));
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Ok(s),
            Ok(None) => {}
            Err(_) => break Err(Diagnostic::GitFailed),
        }
        if start.elapsed() >= limits.timeout {
            break Err(Diagnostic::LimitExceeded);
        }
        thread::sleep(Duration::from_millis(2));
    };
    // Killing the process group also closes inherited pipe handles on failures.
    unsafe {
        libc::kill(-pid, libc::SIGKILL);
    }
    let _ = child.wait();
    let stdout = out
        .join()
        .map_err(|_| Diagnostic::GitFailed)?
        .map_err(|_| Diagnostic::GitFailed)?;
    let stderr = err
        .join()
        .map_err(|_| Diagnostic::GitFailed)?
        .map_err(|_| Diagnostic::GitFailed)?;
    if stdout.len() > max || stderr.len() > max {
        return Err(Diagnostic::LimitExceeded);
    }
    if !status?.success() {
        return Err(Diagnostic::GitFailed);
    }
    Ok(stdout)
}
