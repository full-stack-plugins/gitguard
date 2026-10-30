//! Controller-selected private local bare target only; never a remote-push adapter.
use crate::{
    Repository,
    execution::grant::{GrantAction, GrantExpectation},
    git::runner::{Limits, run, run_with_input, verify_integrity},
    subject::hash,
};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    },
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlatformError {
    UnsafeTarget,
    Unsupported,
    Binding,
    StaleTarget,
    Unavailable,
    Budget,
}
/// Selecting this path and repository label is a protected controller responsibility.
/// This constructor is not an identity provider or permission grant.
pub struct ProtectedBareTarget {
    directory: File,
    repo_id: String,
    config_hash: String,
    format: String,
    identity: String,
    receipt_profile: bool,
}
// The floor is our previously validated baseline, not a claim about the first
// upstream reftable release. No upper-version allowlist substitutes for probing.
fn require_receipt_capabilities(
    version: &[u8],
    probe: impl FnOnce() -> Result<(), PlatformError>,
) -> Result<(), PlatformError> {
    let parse = || {
        let text = std::str::from_utf8(version).ok()?;
        let text = text.strip_prefix("git version ")?.strip_suffix('\n')?;
        let mut components = text.split('.');
        let mut part = || {
            let value = components.next()?;
            if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            value.parse::<u64>().ok()
        };
        let version = (part()?, part()?, part()?);
        for suffix in components {
            if suffix.is_empty()
                || !suffix.bytes().all(|b| b.is_ascii_alphanumeric())
                || suffix.to_ascii_lowercase().starts_with("rc")
            {
                return None;
            }
        }
        Some(version)
    };
    if version.len() > 128 || !parse().is_some_and(|v| v >= (2, 47, 3)) {
        return Err(PlatformError::Unsupported);
    }
    probe().map_err(|_| PlatformError::Unsupported)
}

#[cfg(test)]
fn restricted_probe_command(
    path: &Path,
    args: &[&str],
    input: Option<&[u8]>,
) -> Result<Vec<u8>, PlatformError> {
    probe_command_with_limits(path, args, input, Limits::default())
}

fn probe_command_with_limits(
    path: &Path,
    args: &[&str],
    input: Option<&[u8]>,
    limits: Limits,
) -> Result<Vec<u8>, PlatformError> {
    match input {
        Some(bytes) => run_with_input(path, args, bytes, limits),
        None => run(path, args, limits),
    }
    .map_err(|_| PlatformError::Unsupported)
}

fn probe_reftable_capabilities() -> Result<(), PlatformError> {
    let deadline = Instant::now() + Limits::default().timeout;
    probe_reftable_with(|path, args, input| {
        let timeout = deadline.saturating_duration_since(Instant::now());
        if timeout.is_zero() {
            return Err(PlatformError::Unsupported);
        }
        probe_command_with_limits(
            path,
            args,
            input,
            Limits {
                timeout,
                ..Limits::default()
            },
        )
    })
}

/// Probe only our independent disposable repository, never the selected target.
/// Stable Git plumbing owns the table format; we do not parse binary internals.
fn probe_reftable_with(
    mut command: impl FnMut(&Path, &[&str], Option<&[u8]>) -> Result<Vec<u8>, PlatformError>,
) -> Result<(), PlatformError> {
    let mut probe = || -> Result<(), PlatformError> {
        let directory = tempfile::Builder::new()
            .permissions(std::fs::Permissions::from_mode(0o700))
            .prefix("gitguard-reftable-capability-")
            .tempdir()
            .map_err(|_| PlatformError::Unsupported)?;
        let path = directory.path();
        command(
            path,
            &[
                "-c",
                "user.name=GitGuard capability probe",
                "-c",
                "user.email=probe@example.invalid",
                "init",
                "--bare",
                "--quiet",
                "--object-format=sha1",
                "--ref-format=reftable",
                "--template=",
            ],
            None,
        )?;
        if command(path, &["rev-parse", "--show-object-format"], None)? != b"sha1\n"
            || command(path, &["rev-parse", "--show-ref-format"], None)? != b"reftable\n"
        {
            return Err(PlatformError::Unsupported);
        }
        let oid = |raw: Vec<u8>| -> Result<String, PlatformError> {
            let text = String::from_utf8(raw).map_err(|_| PlatformError::Unsupported)?;
            let value = text.strip_suffix('\n').ok_or(PlatformError::Unsupported)?;
            if value.len() != 40
                || !value
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err(PlatformError::Unsupported);
            }
            Ok(value.to_owned())
        };
        let before = oid(command(
            path,
            &["hash-object", "-w", "--stdin"],
            Some(b"before\n"),
        )?)?;
        let after = oid(command(
            path,
            &["hash-object", "-w", "--stdin"],
            Some(b"after\n"),
        )?)?;
        if before == after {
            return Err(PlatformError::Unsupported);
        }
        let attribution = [
            "-c",
            "user.name=GitGuard capability probe",
            "-c",
            "user.email=probe@example.invalid",
            "update-ref",
            "--stdin",
        ];
        let setup = format!(
            "start\noption no-deref\ncreate refs/gitguard/probe-target {before}\nprepare\ncommit\n"
        );
        if command(path, &attribution, Some(setup.as_bytes()))?
            != b"start: ok\nprepare: ok\ncommit: ok\n"
        {
            return Err(PlatformError::Unsupported);
        }
        let transaction = format!(
            "start\noption no-deref\nupdate refs/gitguard/probe-target {after} {before}\ncreate refs/gitguard/probe-receipt {after}\nprepare\ncommit\n"
        );
        if command(path, &attribution, Some(transaction.as_bytes()))?
            != b"start: ok\nprepare: ok\ncommit: ok\n"
        {
            return Err(PlatformError::Unsupported);
        }
        let refs = [
            "for-each-ref",
            "--format=%(refname) %(objectname)",
            "refs/gitguard/",
        ];
        let expected =
            format!("refs/gitguard/probe-receipt {after}\nrefs/gitguard/probe-target {after}\n")
                .into_bytes();
        if command(path, &refs, None)? != expected {
            return Err(PlatformError::Unsupported);
        }
        let stale = format!(
            "start\noption no-deref\nupdate refs/gitguard/probe-target {before} {before}\ncreate refs/gitguard/probe-stale {after}\nprepare\ncommit\n"
        );
        if command(path, &attribution, Some(stale.as_bytes())).is_ok()
            || command(path, &refs, None)? != expected
        {
            return Err(PlatformError::Unsupported);
        }
        let abort = format!(
            "start\noption no-deref\nupdate refs/gitguard/probe-target {before} {after}\ncreate refs/gitguard/probe-aborted {before}\nprepare\nabort\n"
        );
        if command(path, &attribution, Some(abort.as_bytes()))?
            != b"start: ok\nprepare: ok\nabort: ok\n"
            || command(path, &refs, None)? != expected
        {
            return Err(PlatformError::Unsupported);
        }
        if command(path, &["cat-file", "blob", &after], None)? != b"after\n" {
            return Err(PlatformError::Unsupported);
        }
        Ok(())
    };
    probe().map_err(|_| PlatformError::Unsupported)
}

impl ProtectedBareTarget {
    pub fn open(root: &Path, repo_id: &str) -> Result<Self, PlatformError> {
        Self::open_profile(root, repo_id, false)
    }
    /// Explicit Linux x86_64 / Git >=2.47.3 / probed SHA-1 reftable receipt profile.
    /// Caller must select a private controller-owned bare repository; no migration occurs.
    /// Proves process-crash consistency, not power-loss durability or hosted identity.
    pub fn open_reftable_receipts(root: &Path, repo_id: &str) -> Result<Self, PlatformError> {
        Self::open_profile(root, repo_id, true)
    }
    fn open_profile(
        root: &Path,
        repo_id: &str,
        receipt_profile: bool,
    ) -> Result<Self, PlatformError> {
        if repo_id.is_empty()
            || repo_id.len() > 128
            || !repo_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        {
            return Err(PlatformError::Binding);
        }
        let directory = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(root)
            .map_err(|_| PlatformError::UnsafeTarget)?;
        let mut stat = std::mem::MaybeUninit::<libc::statfs>::uninit();
        if unsafe { libc::fstatfs(directory.as_raw_fd(), stat.as_mut_ptr()) } != 0 {
            return Err(PlatformError::Unavailable);
        }
        if ![0xef53, 0x58465342, 0x9123683e, 0x794c7630, 0x01021994]
            .contains(&unsafe { stat.assume_init() }.f_type)
        {
            return Err(PlatformError::Unsupported);
        }
        let mut target = Self {
            directory,
            repo_id: repo_id.into(),
            config_hash: String::new(),
            format: String::new(),
            identity: String::new(),
            receipt_profile,
        };
        target.scan()?;
        let config = target.config()?;
        // Parse explicit frozen bytes in isolated metadata, never load unvalidated target config.
        let temp = tempfile::tempdir().map_err(|_| PlatformError::Unavailable)?;
        let file = temp.path().join("config-source");
        std::fs::write(&file, &config).map_err(|_| PlatformError::Unavailable)?;
        let values = run(
            temp.path(),
            &[
                "config",
                "--file",
                file.to_str().ok_or(PlatformError::Unsupported)?,
                "--no-includes",
                "--null",
                "--list",
            ],
            Limits::default(),
        )
        .map_err(|_| PlatformError::UnsafeTarget)?;
        let mut map = std::collections::BTreeMap::new();
        for field in values.split(|b| *b == 0).filter(|b| !b.is_empty()) {
            let (k, v) = field.split_once_byte().ok_or(PlatformError::UnsafeTarget)?;
            if map.insert(k.to_vec(), v.to_vec()).is_some() {
                return Err(PlatformError::UnsafeTarget);
            }
        }
        if map.keys().any(|k| {
            !(receipt_profile && k.as_slice() == b"extensions.refstorage")
                && ![
                    b"core.repositoryformatversion".as_slice(),
                    b"core.filemode",
                    b"core.bare",
                    b"extensions.objectformat",
                ]
                .contains(&k.as_slice())
        }) || map.get(b"core.bare".as_slice()).map(Vec::as_slice) != Some(b"true")
            || !matches!(
                map.get(b"core.filemode".as_slice()).map(Vec::as_slice),
                Some(b"true" | b"false")
            )
        {
            return Err(PlatformError::Unsupported);
        }
        target.format = if receipt_profile {
            if map
                .get(b"extensions.refstorage".as_slice())
                .map(Vec::as_slice)
                != Some(b"reftable")
                || map
                    .get(b"core.repositoryformatversion".as_slice())
                    .map(Vec::as_slice)
                    != Some(b"1")
                || map.contains_key(b"extensions.objectformat".as_slice())
            {
                return Err(PlatformError::Unsupported);
            }
            "sha1"
        } else {
            match (
                map.get(b"core.repositoryformatversion".as_slice())
                    .map(Vec::as_slice),
                map.get(b"extensions.objectformat".as_slice())
                    .map(Vec::as_slice),
            ) {
                (Some(b"0"), None) => "sha1",
                (Some(b"1"), Some(b"sha256")) => "sha256",
                _ => return Err(PlatformError::Unsupported),
            }
        }
        .into();
        if receipt_profile {
            if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
                return Err(PlatformError::Unsupported);
            }
            let version = run(temp.path(), &["version"], Limits::default())
                .map_err(|_| PlatformError::Unsupported)?;
            require_receipt_capabilities(&version, probe_reftable_capabilities)?;
        }
        target.config_hash = hash(&config);
        let m = target
            .directory
            .metadata()
            .map_err(|_| PlatformError::Unavailable)?;
        target.identity = guardengine::digest_json(&(
            "gitguard.local-bare/v1alpha1",
            repo_id,
            m.dev(),
            m.ino(),
            &target.config_hash,
            &target.format,
        ))
        .map_err(|_| PlatformError::Binding)?;
        Ok(target)
    }
    // The descriptor stays open in this parent; child procfs lookup survives its own CLOEXEC.
    fn path(&self) -> PathBuf {
        PathBuf::from(format!(
            "/proc/{}/fd/{}",
            std::process::id(),
            self.directory.as_raw_fd()
        ))
    }
    pub(crate) fn identity(&self) -> &str {
        &self.identity
    }
    fn config(&self) -> Result<Vec<u8>, PlatformError> {
        let f = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(self.path().join("config"))
            .map_err(|_| PlatformError::UnsafeTarget)?;
        let m = f.metadata().map_err(|_| PlatformError::Unavailable)?;
        if !m.is_file()
            || m.len() > 65536
            || m.nlink() != 1
            || m.uid() != unsafe { libc::geteuid() }
            || m.mode() & 0o022 != 0
        {
            return Err(PlatformError::UnsafeTarget);
        }
        let mut b = vec![];
        f.take(65537)
            .read_to_end(&mut b)
            .map_err(|_| PlatformError::Unavailable)?;
        if b.len() > 65536 {
            return Err(PlatformError::Budget);
        }
        Ok(b)
    }
    pub(crate) fn unchanged(&self) -> Result<(), PlatformError> {
        let m = self
            .directory
            .metadata()
            .map_err(|_| PlatformError::Unavailable)?;
        if m.uid() != unsafe { libc::geteuid() }
            || m.mode() & 0o077 != 0
            || hash(&self.config()?) != self.config_hash
        {
            return Err(PlatformError::UnsafeTarget);
        }
        Ok(())
    }
    fn scan(&self) -> Result<(), PlatformError> {
        let root = self.path();
        let m = self
            .directory
            .metadata()
            .map_err(|_| PlatformError::Unavailable)?;
        if m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o077 != 0 {
            return Err(PlatformError::UnsafeTarget);
        }
        let mut pending = vec![(root.clone(), 0usize)];
        let (mut entries, mut bytes, mut paths) = (0usize, 0u64, 0usize);
        let deadline = Instant::now() + Duration::from_secs(2);
        while let Some((path, depth)) = pending.pop() {
            if depth > 32 {
                return Err(PlatformError::Budget);
            }
            for item in std::fs::read_dir(path).map_err(|_| PlatformError::UnsafeTarget)? {
                if Instant::now() > deadline {
                    return Err(PlatformError::Budget);
                }
                let item = item.map_err(|_| PlatformError::Unavailable)?;
                entries += 1;
                if entries > 10000 {
                    return Err(PlatformError::Budget);
                }
                let path = item.path();
                paths = paths
                    .checked_add(path.as_os_str().as_encoded_bytes().len())
                    .ok_or(PlatformError::Budget)?;
                if paths > 1024 * 1024 {
                    return Err(PlatformError::Budget);
                }
                let relative = path
                    .strip_prefix(&root)
                    .map_err(|_| PlatformError::UnsafeTarget)?;
                if [
                    "commondir",
                    "shallow",
                    "info/grafts",
                    "objects/info/alternates",
                    "objects/info/http-alternates",
                    "refs/replace",
                    "worktrees",
                ]
                .iter()
                .any(|s| relative == Path::new(s))
                {
                    return Err(PlatformError::Unsupported);
                }
                let m =
                    std::fs::symlink_metadata(&path).map_err(|_| PlatformError::UnsafeTarget)?;
                if m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o022 != 0 {
                    return Err(PlatformError::UnsafeTarget);
                }
                if m.is_dir() {
                    pending.push((path, depth + 1));
                } else if m.is_file() && m.nlink() == 1 {
                    bytes = bytes.checked_add(m.len()).ok_or(PlatformError::Budget)?;
                    if bytes > 64 * 1024 * 1024 {
                        return Err(PlatformError::Budget);
                    }
                } else {
                    return Err(PlatformError::UnsafeTarget);
                }
            }
        }
        Ok(())
    }
    fn git(&self, args: &[&str]) -> Result<Vec<u8>, PlatformError> {
        run(&self.path(), args, Limits::default()).map_err(|_| PlatformError::Unavailable)
    }
    pub(crate) fn preflight(
        &self,
        repository: &Repository,
        expected: &GrantExpectation,
    ) -> Result<(), PlatformError> {
        self.unchanged()?;
        self.scan()?;
        if repository.repo_id() != self.repo_id
            || expected.repo_id() != self.repo_id
            || repository.object_format() != self.format
        {
            return Err(PlatformError::Binding);
        }
        verify_integrity(&self.path(), repository.git_version(), Limits::default())
            .map_err(|_| PlatformError::Unavailable)?;
        if expected.request().action != GrantAction::Push
            && self.git(&["cat-file", "-t", expected.candidate_oid()])? != b"commit\n"
        {
            return Err(PlatformError::Binding);
        }
        let req = expected.request();
        let raw = self.git(&[
            "for-each-ref",
            "--format=%(refname) %(objectname) %(symref)",
            &req.target,
        ])?;
        let text = std::str::from_utf8(&raw).map_err(|_| PlatformError::Binding)?;
        let mut actual = None;
        for line in text.lines() {
            let fields: Vec<_> = line.splitn(3, ' ').collect();
            if fields.first() == Some(&req.target.as_str()) {
                if fields.len() != 3 || !fields[2].is_empty() {
                    return Err(PlatformError::Unsupported);
                }
                actual = Some(fields[1]);
            }
        }
        if actual != req.expected_target_oid.as_deref() {
            return Err(PlatformError::StaleTarget);
        }
        if let Some(old) = req.expected_target_oid.as_deref()
            && self.git(&["cat-file", "-t", old])? != b"commit\n"
        {
            return Err(PlatformError::Binding);
        }
        if matches!(req.action, GrantAction::Merge | GrantAction::Push) {
            let args = [
                "merge-base",
                "--is-ancestor",
                req.expected_target_oid
                    .as_deref()
                    .ok_or(PlatformError::Binding)?,
                expected.candidate_oid(),
            ];
            if req.action == GrantAction::Push {
                repository.run(&args).map_err(|_| PlatformError::Binding)?;
            } else {
                self.git(&args)?;
            }
        }
        Ok(())
    }
    /// Transfer only the frozen candidate's reachable objects. No network/helper is enabled.
    /// Called only after a durable attempt claim; failures leave the attempt uncertain.
    pub(crate) fn transfer_candidate(
        &self,
        repository: &Repository,
        expected: &GrantExpectation,
    ) -> Result<(), PlatformError> {
        self.unchanged()?;
        self.scan()?;
        let staging = tempfile::tempdir().map_err(|_| PlatformError::Unavailable)?;
        run(
            staging.path(),
            &[
                "init",
                "--bare",
                "--quiet",
                &format!("--object-format={}", self.format),
            ],
            Limits::default(),
        )
        .map_err(|_| PlatformError::Unavailable)?;
        crate::git::repository::copy_objects(
            &repository.store.path().join("objects"),
            &staging.path().join("objects"),
            &mut (64 * 1024 * 1024),
            &mut 0,
        )
        .map_err(|_| PlatformError::Budget)?;
        run(
            staging.path(),
            &[
                "update-ref",
                "refs/heads/candidate",
                expected.candidate_oid(),
            ],
            Limits::default(),
        )
        .map_err(|_| PlatformError::Binding)?;
        // Output is bounded by the unchanged runner (8 MiB). No arbitrary source refs are packed.
        let pack = run(
            staging.path(),
            &[
                "pack-objects",
                "--all",
                "--stdout",
                "--threads=1",
                "--window=0",
                "--depth=0",
            ],
            Limits::default(),
        )
        .map_err(|_| PlatformError::Budget)?;
        let pack_file = staging.path().join("transfer.pack");
        std::fs::write(&pack_file, &pack).map_err(|_| PlatformError::Unavailable)?;
        let oid = run(
            staging.path(),
            &[
                "index-pack",
                "--strict",
                "--threads=1",
                pack_file.to_str().ok_or(PlatformError::Unsupported)?,
            ],
            Limits::default(),
        )
        .map_err(|_| PlatformError::Binding)?;
        let oid = std::str::from_utf8(&oid)
            .map_err(|_| PlatformError::Binding)?
            .trim();
        if oid.len() != expected.candidate_oid().len()
            || !oid
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            return Err(PlatformError::Binding);
        }
        self.unchanged()?;
        self.scan()?;
        let destination = self.path().join("objects/pack");
        for (extension, source) in [
            ("pack", pack_file),
            ("idx", staging.path().join("transfer.idx")),
        ] {
            let file = File::open(source).map_err(|_| PlatformError::Unavailable)?;
            if file
                .metadata()
                .map_err(|_| PlatformError::Unavailable)?
                .len()
                > 8 * 1024 * 1024
            {
                return Err(PlatformError::Budget);
            }
            let mut bytes = Vec::new();
            file.take(8 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| PlatformError::Unavailable)?;
            if bytes.len() > 8 * 1024 * 1024 {
                return Err(PlatformError::Budget);
            }
            let final_path = destination.join(format!("pack-{oid}.{extension}"));
            let mut temporary = tempfile::NamedTempFile::new_in(&destination)
                .map_err(|_| PlatformError::Unavailable)?;
            temporary
                .write_all(&bytes)
                .map_err(|_| PlatformError::Unavailable)?;
            temporary
                .as_file()
                .sync_all()
                .map_err(|_| PlatformError::Unavailable)?;
            if let Err(error) = temporary.persist_noclobber(&final_path)
                && (error.error.kind() != std::io::ErrorKind::AlreadyExists
                    || std::fs::read(&final_path).map_err(|_| PlatformError::Unavailable)? != bytes)
            {
                return Err(PlatformError::UnsafeTarget);
            }
        }
        File::open(destination)
            .and_then(|f| f.sync_all())
            .map_err(|_| PlatformError::Unavailable)?;
        self.scan()?;
        verify_integrity(&self.path(), repository.git_version(), Limits::default())
            .map_err(|_| PlatformError::Binding)?;
        if self.git(&["cat-file", "-t", expected.candidate_oid()])? != b"commit\n" {
            return Err(PlatformError::Binding);
        }
        Ok(())
    }
    pub(crate) fn compare_and_swap(
        &self,
        expected: &GrantExpectation,
        request_digest: &str,
    ) -> Result<(), PlatformError> {
        self.unchanged()?;
        if self.receipt_profile {
            self.scan()?;
            let bytes = self.receipt_bytes(expected, request_digest)?;
            let object = run_with_input(
                &self.path(),
                &["hash-object", "-w", "--stdin"],
                &bytes,
                Limits::default(),
            )
            .map_err(|_| PlatformError::Unavailable)?;
            let oid = std::str::from_utf8(&object)
                .map_err(|_| PlatformError::Binding)?
                .trim();
            if oid.len() != 40
                || !oid
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err(PlatformError::Binding);
            }
            let req = expected.request();
            let transaction = format!(
                "start\noption no-deref\nupdate {} {} {}\ncreate {} {}\nprepare\ncommit\n",
                req.target,
                expected.candidate_oid(),
                req.expected_target_oid
                    .as_deref()
                    .unwrap_or("0000000000000000000000000000000000000000"),
                self.receipt_ref(expected),
                oid
            );
            // Fixed reflog attribution only, never an actor identity or grant.
            // Avoid Git's auto-email hostname lookup under the unchanged no-network profile.
            let output = run_with_input(
                &self.path(),
                &[
                    "-c",
                    "user.name=GitGuard local receipt",
                    "-c",
                    "user.email=local-receipt@example.invalid",
                    "update-ref",
                    "--stdin",
                ],
                transaction.as_bytes(),
                Limits::default(),
            )
            .map_err(|_| PlatformError::Unavailable)?;
            if output != b"start: ok\nprepare: ok\ncommit: ok\n" {
                return Err(PlatformError::Unavailable);
            }
            return Ok(());
        }
        let req = expected.request();
        let absent = "0".repeat(expected.candidate_oid().len());
        let output = self.git(&[
            "update-ref",
            "--no-deref",
            &req.target,
            expected.candidate_oid(),
            req.expected_target_oid.as_deref().unwrap_or(&absent),
        ])?;
        if !output.is_empty() {
            return Err(PlatformError::Unavailable);
        }
        Ok(())
    }
    fn receipt_ref(&self, expected: &GrantExpectation) -> String {
        // Hash arbitrary bounded operation IDs; no request-controlled ref syntax.
        format!(
            "refs/gitguard/receipts/{}",
            hash(expected.operation_id().as_bytes()).trim_start_matches("sha256:")
        )
    }
    fn receipt_bytes(
        &self,
        expected: &GrantExpectation,
        request_digest: &str,
    ) -> Result<Vec<u8>, PlatformError> {
        serde_json::to_vec(&(
            "gitguard.local-reftable-receipt/v1alpha1",
            self.identity(),
            expected.operation_id(),
            expected.digest(),
            request_digest,
            expected.candidate_oid(),
        ))
        .map_err(|_| PlatformError::Binding)
    }
    /// Positive proof only. Missing, malformed or inaccessible receipt never means not written.
    pub(crate) fn confirms_receipt(
        &self,
        expected: &GrantExpectation,
        request_digest: &str,
    ) -> Result<bool, PlatformError> {
        if !self.receipt_profile {
            return Ok(false);
        }
        self.unchanged()?;
        self.scan()?;
        let name = self.receipt_ref(expected);
        let raw = self.git(&[
            "for-each-ref",
            "--format=%(refname) %(objectname) %(symref)",
            &name,
        ])?;
        let text = std::str::from_utf8(&raw).map_err(|_| PlatformError::Binding)?;
        let fields: Vec<_> = text.trim_end_matches('\n').split(' ').collect();
        if fields.len() != 3
            || fields[0] != name
            || !fields[2].is_empty()
            || fields[1].len() != 40
            || !fields[1]
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Ok(false);
        }
        let bytes = run(
            &self.path(),
            &["cat-file", "blob", fields[1]],
            Limits {
                output_bytes: 16_384,
                ..Limits::default()
            },
        );
        Ok(bytes.is_ok_and(|b| {
            self.receipt_bytes(expected, request_digest)
                .is_ok_and(|wanted| b == wanted)
        }))
    }
}
trait SplitConfig {
    fn split_once_byte(&self) -> Option<(&[u8], &[u8])>;
}
impl SplitConfig for [u8] {
    fn split_once_byte(&self) -> Option<(&[u8], &[u8])> {
        self.iter()
            .position(|b| *b == b'\n')
            .map(|i| (&self[..i], &self[i + 1..]))
    }
}

#[cfg(test)]
mod receipt_capability_tests {
    use super::*;

    #[test]
    fn minimum_and_current_versions_need_real_capabilities() {
        for version in [
            b"git version 2.47.3\n".as_slice(),
            b"git version 2.56.0\n",
            b"git version 3.0.0\n",
            b"git version 2.56.0.distro1\n",
            b"git version 2.47.3.ubuntu.1\n",
        ] {
            assert!(require_receipt_capabilities(version, || Ok(())).is_ok());
            assert_eq!(
                require_receipt_capabilities(version, || Err(PlatformError::Unavailable)),
                Err(PlatformError::Unsupported)
            );
        }
        for version in [
            b"git version 2.47.2\n".as_slice(),
            b"git version 2.46.9\n",
            b"git version 1.99.0\n",
            b"git version 2.56\n",
            b"git version latest\n",
            b"git version 2.56.0 extra\n",
            b"git version 2.56.0-rc1\n",
            b"git version 2.56.0.rc1\n",
            b"git version 2.56.0..distro\n",
        ] {
            assert_eq!(
                require_receipt_capabilities(version, || panic!(
                    "below minimum/malformed must not probe"
                )),
                Err(PlatformError::Unsupported)
            );
        }
    }

    #[test]
    fn oversized_version_cannot_trigger_a_probe() {
        assert_eq!(
            require_receipt_capabilities(&vec![b'9'; 129], || panic!("unbounded version")),
            Err(PlatformError::Unsupported)
        );
    }

    #[test]
    fn real_reftable_probe_checks_transactions_without_touching_user_targets() {
        assert!(probe_reftable_capabilities().is_ok());
    }

    #[test]
    fn missing_capability_and_inconsistent_backend_fail_closed() {
        assert_eq!(
            probe_reftable_with(|_, _, _| Err(PlatformError::Unavailable)),
            Err(PlatformError::Unsupported)
        );
        assert_eq!(
            probe_reftable_with(|path, args, input| {
                if args == ["rev-parse", "--show-ref-format"] {
                    return Ok(b"files\n".to_vec());
                }
                restricted_probe_command(path, args, input)
            }),
            Err(PlatformError::Unsupported)
        );
        assert_eq!(
            probe_reftable_with(|path, args, input| {
                if args.contains(&"update-ref") && input.is_some() {
                    return Ok(b"success without transaction acknowledgement\n".to_vec());
                }
                restricted_probe_command(path, args, input)
            }),
            Err(PlatformError::Unsupported)
        );
    }

    #[test]
    fn receipt_profile_keeps_exact_sha1_and_configuration_constraints() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::Builder::new()
            .permissions(std::fs::Permissions::from_mode(0o700))
            .tempdir()
            .unwrap();
        run(
            root.path(),
            &[
                "-c",
                "user.name=GitGuard capability probe",
                "-c",
                "user.email=probe@example.invalid",
                "init",
                "--bare",
                "--quiet",
                "--ref-format=reftable",
                "--object-format=sha256",
            ],
            Limits::default(),
        )
        .unwrap();
        assert!(matches!(
            ProtectedBareTarget::open_reftable_receipts(root.path(), "repo"),
            Err(PlatformError::Unsupported)
        ));
    }
}
