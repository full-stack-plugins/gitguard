//! Controller-selected private local bare target only; never a remote-push adapter.
use crate::{
    Repository,
    execution::grant::{GrantAction, GrantExpectation},
    git::runner::{Limits, run},
    subject::hash,
};
use std::{
    fs::{File, OpenOptions},
    io::Read,
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, OpenOptionsExt},
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
}
impl ProtectedBareTarget {
    pub fn open(root: &Path, repo_id: &str) -> Result<Self, PlatformError> {
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
            ![
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
        target.format = match (
            map.get(b"core.repositoryformatversion".as_slice())
                .map(Vec::as_slice),
            map.get(b"extensions.objectformat".as_slice())
                .map(Vec::as_slice),
        ) {
            (Some(b"0"), None) => "sha1",
            (Some(b"1"), Some(b"sha256")) => "sha256",
            _ => return Err(PlatformError::Unsupported),
        }
        .into();
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
        self.git(&["fsck", "--full", "--strict", "--no-reflogs"])?;
        if self.git(&["cat-file", "-t", expected.candidate_oid()])? != b"commit\n" {
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
        if req.action == GrantAction::Merge {
            self.git(&[
                "merge-base",
                "--is-ancestor",
                req.expected_target_oid
                    .as_deref()
                    .ok_or(PlatformError::Binding)?,
                expected.candidate_oid(),
            ])?;
        }
        Ok(())
    }
    pub(crate) fn compare_and_swap(
        &self,
        expected: &GrantExpectation,
    ) -> Result<(), PlatformError> {
        self.unchanged()?;
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
