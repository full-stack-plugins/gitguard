//! Linux single-process Git profile; no host-wide subreaper or signal disposition changes.
use super::runner::Limits;
use crate::{Diagnostic, Result};
use std::process::Command;

pub(super) fn execute(command: Command, limits: Limits) -> Result<Vec<u8>> {
    let maximum = Limits::default();
    if limits.output_bytes == 0
        || limits.output_bytes > maximum.output_bytes
        || limits.timeout.is_zero()
        || limits.timeout > maximum.timeout
        || limits.storage_bytes > maximum.storage_bytes
    {
        return Err(Diagnostic::LimitExceeded);
    }
    #[cfg(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    {
        linux::execute(command, limits)
    }
    #[cfg(not(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )))]
    {
        let _ = command;
        Err(Diagnostic::UnsupportedProcessProfile)
    }
}

#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod linux {
    use super::*;
    use std::{
        io::{self, Read},
        os::{
            fd::AsRawFd,
            unix::process::{CommandExt, ExitStatusExt},
        },
        process::{Child, ExitStatus},
        thread,
        time::{Duration, Instant},
    };
    const CLEANUP: Duration = Duration::from_millis(250);
    fn statement(code: u16, k: u32) -> libc::sock_filter {
        libc::sock_filter {
            code,
            jt: 0,
            jf: 0,
            k,
        }
    }
    fn equal(k: u32, jt: u8, jf: u8) -> libc::sock_filter {
        libc::sock_filter {
            code: 0x15,
            jt,
            jf,
            k,
        }
    }
    fn filter() -> Vec<libc::sock_filter> {
        #[cfg(target_arch = "x86_64")]
        const ARCH: u32 = 0xc000003e;
        #[cfg(target_arch = "aarch64")]
        const ARCH: u32 = 0xc00000b7;
        // seccomp_data: nr at0, arch at4. Kill unexpected ABI; x32 shares AUDIT_ARCH_X86_64.
        let mut f = vec![
            statement(0x20, 4),
            equal(ARCH, 1, 0),
            statement(0x06, 0x80000000),
            statement(0x20, 0),
        ];
        #[cfg(target_arch = "x86_64")]
        f.extend([
            libc::sock_filter {
                code: 0x35,
                jt: 0,
                jf: 1,
                k: 0x40000000,
            },
            statement(0x06, 0x80000000),
        ]);
        let denied = [
            libc::SYS_clone,
            libc::SYS_clone3,
            libc::SYS_socket,
            libc::SYS_socketpair,
            libc::SYS_connect,
            libc::SYS_execveat,
        ];
        for nr in denied {
            f.extend([equal(nr as u32, 0, 1), statement(0x06, 0x80000000)]);
        }
        #[cfg(target_arch = "x86_64")]
        for nr in [libc::SYS_fork, libc::SYS_vfork] {
            f.extend([equal(nr as u32, 0, 1), statement(0x06, 0x80000000)]);
        }
        f.push(statement(0x06, 0x7fff0000));
        f
    }
    fn nonblocking(fd: i32) -> io::Result<()> {
        // Owned child pipe descriptor; no concurrent reader or flag mutation.
        unsafe {
            let flags = libc::fcntl(fd, libc::F_GETFL);
            if flags < 0 || libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }
    fn drain(input: &mut impl Read, keep: &mut Vec<u8>, left: &mut usize) -> Result<bool> {
        // One bounded read per stream per turn: even a continuous writer cannot starve deadlines.
        let mut chunk = [0u8; 8192];
        match input.read(&mut chunk) {
            Ok(0) => Ok(true),
            Ok(n) => {
                if n > *left {
                    return Err(Diagnostic::LimitExceeded);
                }
                *left -= n;
                keep.extend_from_slice(&chunk[..n]);
                Ok(false)
            }
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) =>
            {
                Ok(false)
            }
            Err(_) => Err(Diagnostic::GitFailed),
        }
    }
    fn cleanup(child: &mut Child) -> Result<()> {
        let end = Instant::now() + CLEANUP;
        loop {
            if child
                .try_wait()
                .map_err(|_| Diagnostic::ProcessCleanupFailed)?
                .is_some()
            {
                return Ok(());
            }
            child.kill().map_err(|_| Diagnostic::ProcessCleanupFailed)?;
            if Instant::now() >= end {
                return Err(Diagnostic::ProcessCleanupFailed);
            }
            thread::sleep(Duration::from_millis(1));
        }
    }
    pub(super) fn execute(mut command: Command, limits: Limits) -> Result<Vec<u8>> {
        let filter = filter();
        // No allocation/locks/Rust runtime work after fork: only setrlimit/prctl and errno conversion.
        unsafe {
            command.pre_exec(move || {
                let program = libc::sock_fprog {
                    len: filter.len() as u16,
                    filter: filter.as_ptr() as *mut _,
                };
                let core = libc::rlimit {
                    rlim_cur: 0,
                    rlim_max: 0,
                };
                if libc::setrlimit(libc::RLIMIT_CORE, &core) != 0
                    || libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0
                    || libc::prctl(libc::PR_SET_SECCOMP, 2, &program) != 0
                {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let mut child = command
            .spawn()
            .map_err(|_| Diagnostic::UnsupportedProcessProfile)?;
        let result = (|| {
            let mut out = child.stdout.take().ok_or(Diagnostic::GitFailed)?;
            let mut err = child.stderr.take().ok_or(Diagnostic::GitFailed)?;
            nonblocking(out.as_raw_fd()).map_err(|_| Diagnostic::GitFailed)?;
            nonblocking(err.as_raw_fd()).map_err(|_| Diagnostic::GitFailed)?;
            let mut stdout = vec![];
            let mut stderr = vec![];
            let mut left = limits.output_bytes;
            let mut out_done = false;
            let mut err_done = false;
            let mut status: Option<ExitStatus> = None;
            let started = Instant::now();
            loop {
                if started.elapsed() >= limits.timeout {
                    return Err(Diagnostic::LimitExceeded);
                }
                if !out_done {
                    out_done = drain(&mut out, &mut stdout, &mut left)?;
                }
                if !err_done {
                    err_done = drain(&mut err, &mut stderr, &mut left)?;
                }
                if status.is_none() {
                    status = child.try_wait().map_err(|_| Diagnostic::GitFailed)?;
                }
                if let Some(status) = status
                    && out_done
                    && err_done
                {
                    return if status.signal() == Some(libc::SIGSYS) {
                        Err(Diagnostic::ProcessPolicyDenied)
                    } else if status.success() {
                        Ok(stdout)
                    } else {
                        Err(Diagnostic::GitFailed)
                    };
                }
                thread::sleep(Duration::from_millis(1));
            }
        })();
        cleanup(&mut child)?;
        result
    }
}

#[cfg(all(
    test,
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod tests {
    use super::*;
    use std::{
        process::Stdio,
        time::{Duration, Instant},
    };
    fn fixture(script: &str) -> Command {
        // Fixed private process-supervision fixtures; never exposed through GitGuard API.
        let mut c = Command::new("/usr/bin/python3");
        c.args(["-I", "-S", "-c", script])
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        c
    }
    #[test]
    fn core_dumps_cannot_export_git_memory_or_secret_diagnostics() {
        let c = fixture("import resource;print(resource.getrlimit(resource.RLIMIT_CORE))");
        assert_eq!(execute(c, Limits::default()).unwrap(), b"(0, 0)\n");
    }
    #[test]
    fn combined_stdout_stderr_budget_is_enforced() {
        let c = fixture("import os; os.write(1,b'123'); os.write(2,b'456')");
        assert_eq!(
            execute(
                c,
                Limits {
                    output_bytes: 5,
                    ..Limits::default()
                }
            ),
            Err(Diagnostic::LimitExceeded)
        );
        let c = fixture("import os; os.write(1,b'123'); os.write(2,b'456')");
        assert_eq!(
            execute(
                c,
                Limits {
                    output_bytes: 6,
                    ..Limits::default()
                }
            )
            .unwrap(),
            b"123"
        );
    }
    #[test]
    fn actual_fork_and_network_socket_creation_are_denied() {
        for script in ["import os;os.fork()", "import socket;socket.socket()"] {
            assert_eq!(
                execute(fixture(script), Limits::default()),
                Err(Diagnostic::ProcessPolicyDenied)
            );
        }
    }
    #[test]
    #[cfg(target_arch = "x86_64")]
    fn x32_syscall_namespace_is_terminated_instead_of_bypassing_filter() {
        let c = fixture("import ctypes; ctypes.CDLL(None).syscall(0x40000000+39); print('bypass')");
        assert_eq!(
            execute(c, Limits::default()),
            Err(Diagnostic::ProcessPolicyDenied)
        );
    }
    #[test]
    fn flood_is_stopped_before_deadline_without_waiting_for_reader_threads() {
        let start = Instant::now();
        let c = fixture(
            "import os\nwhile True:\n try: os.write(1,b'x'*8192)\n except BrokenPipeError: pass",
        );
        assert_eq!(
            execute(
                c,
                Limits {
                    output_bytes: 128,
                    timeout: Duration::from_secs(2),
                    ..Limits::default()
                }
            ),
            Err(Diagnostic::LimitExceeded)
        );
        assert!(start.elapsed() < Duration::from_secs(1));
    }
    #[test]
    fn timed_out_process_is_reaped_and_cannot_retain_pipe_handles() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pid");
        let mut c = fixture(
            "import os,sys,time\nopen(sys.argv[1],'w').write(str(os.getpid()))\ntime.sleep(60)",
        );
        c.arg(&path);
        let start = Instant::now();
        assert_eq!(
            execute(
                c,
                Limits {
                    timeout: Duration::from_millis(100),
                    ..Limits::default()
                }
            ),
            Err(Diagnostic::LimitExceeded)
        );
        assert!(start.elapsed() < Duration::from_secs(1));
        let pid = std::fs::read_to_string(path).unwrap();
        assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
    }
}
