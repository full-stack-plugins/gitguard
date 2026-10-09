use super::runner::{Limits, run};
use crate::{Diagnostic, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};
pub struct Repository {
    root: PathBuf,
    common: PathBuf,
    pub(crate) gitdir: PathBuf,
    pub(crate) store: tempfile::TempDir,
    format: String,
    git_version: String,
    pub(crate) repo_id: String,
    pub(crate) limits: Limits,
}
fn small(path: &Path) -> Result<Vec<u8>> {
    let m = fs::symlink_metadata(path)?;
    if !m.is_file() || m.len() > 65536 {
        return Err(Diagnostic::UnsafeStorage);
    }
    Ok(fs::read(path)?)
}
fn copy_objects(from: &Path, to: &Path, left: &mut u64, count: &mut usize) -> Result<()> {
    fs::create_dir_all(to)?;
    for e in fs::read_dir(from)? {
        let e = e?;
        let m = e.file_type()?;
        *count += 1;
        if *count > 100000 {
            return Err(Diagnostic::LimitExceeded);
        }
        if m.is_symlink() {
            return Err(Diagnostic::UnsafeStorage);
        }
        if m.is_dir() {
            copy_objects(&e.path(), &to.join(e.file_name()), left, count)?;
        } else if m.is_file() {
            let size = e.metadata()?.len();
            *left = left.checked_sub(size).ok_or(Diagnostic::LimitExceeded)?;
            let bytes = fs::read(e.path())?;
            if bytes.len() as u64 != size {
                return Err(Diagnostic::UnsafeStorage);
            }
            fs::write(to.join(e.file_name()), bytes)?;
        } else {
            return Err(Diagnostic::UnsafeStorage);
        }
    }
    Ok(())
}
fn repository_format(config: &[u8]) -> Result<String> {
    let mut version = None;
    let mut format = None;
    for record in config.split(|b| *b == 0).filter(|r| !r.is_empty()) {
        let (key, value) = match record.iter().position(|b| *b == b'\n') {
            Some(i) => (&record[..i], &record[i + 1..]),
            None => (record, &b""[..]),
        };
        let key = std::str::from_utf8(key).map_err(|_| Diagnostic::InvalidRepository)?;
        match key {
            "core.repositoryformatversion" => {
                if version.is_some() {
                    return Err(Diagnostic::UnsupportedFormat);
                }
                version = Some(match value {
                    b"0" => 0,
                    b"1" => 1,
                    _ => return Err(Diagnostic::UnsupportedFormat),
                });
            }
            "extensions.objectformat" => {
                if format.is_some() {
                    return Err(Diagnostic::UnsupportedFormat);
                }
                format = Some(match value {
                    b"sha1" => "sha1",
                    b"sha256" => "sha256",
                    _ => return Err(Diagnostic::UnsupportedFormat),
                });
            }
            "core.worktree" => return Err(Diagnostic::UnsafeStorage),
            "core.bare" if value != b"false" => return Err(Diagnostic::InvalidRepository),
            _ if key.starts_with("extensions.") => return Err(Diagnostic::UnsupportedFormat),
            _ if key == "include.path" || key.starts_with("includeif.") => {
                return Err(Diagnostic::UnsafeStorage);
            }
            _ => {}
        }
    }
    if format.is_some() && version != Some(1) {
        return Err(Diagnostic::UnsupportedFormat);
    }
    Ok(format.unwrap_or("sha1").into())
}
impl Repository {
    pub fn discover(root: &Path, repo_id: &str) -> Result<Self> {
        Self::discover_with_limits(root, repo_id, Limits::default())
    }
    pub fn discover_with_limits(root: &Path, repo_id: &str, limits: Limits) -> Result<Self> {
        if repo_id.is_empty()
            || repo_id.len() > 128
            || !repo_id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
        {
            return Err(Diagnostic::InvalidRepository);
        }
        let root = fs::canonicalize(root)?;
        let dot = root.join(".git");
        if fs::symlink_metadata(&dot)?.file_type().is_symlink() {
            return Err(Diagnostic::UnsafeStorage);
        }
        let gitdir = if dot.is_dir() {
            fs::canonicalize(dot)?
        } else {
            let b = small(&dot)?;
            let s = std::str::from_utf8(&b).map_err(|_| Diagnostic::InvalidRepository)?;
            let p = s
                .trim()
                .strip_prefix("gitdir: ")
                .ok_or(Diagnostic::InvalidRepository)?;
            fs::canonicalize(root.join(p))?
        };
        let common = if gitdir.join("commondir").exists() {
            let b = small(&gitdir.join("commondir"))?;
            let s = std::str::from_utf8(&b).map_err(|_| Diagnostic::InvalidRepository)?;
            fs::canonicalize(gitdir.join(s.trim()))?
        } else {
            gitdir.clone()
        };
        let config = small(&common.join("config"))?;
        let store = tempfile::tempdir()?;
        let frozen_config = store.path().join("source-config");
        fs::write(&frozen_config, &config)?;
        // Parse a frozen explicit file; never load source metadata as Git's own config.
        // Native parsing supplies section, quoting, escaping and comment semantics.
        let config_values = run(
            store.path(),
            &[
                "config",
                "--file",
                frozen_config
                    .to_str()
                    .ok_or(Diagnostic::InvalidRepository)?,
                "--no-includes",
                "--null",
                "--list",
            ],
            limits,
        )?;
        let format = repository_format(&config_values)?;
        fs::remove_file(frozen_config)?;
        let git_version = String::from_utf8(run(store.path(), &["version"], limits)?)
            .map_err(|_| Diagnostic::GitFailed)?
            .trim()
            .to_owned();
        if !git_version.starts_with("git version ") || git_version.len() > 128 {
            return Err(Diagnostic::GitFailed);
        }
        let objects = common.join("objects");
        if fs::symlink_metadata(&objects)?.file_type().is_symlink() {
            return Err(Diagnostic::UnsafeStorage);
        }
        for name in ["alternates", "http-alternates"] {
            if objects.join("info").join(name).exists() {
                return Err(Diagnostic::UnsafeStorage);
            }
        }
        run(
            store.path(),
            &[
                "init",
                "--bare",
                "--quiet",
                &format!("--object-format={format}"),
            ],
            limits,
        )?;
        copy_objects(
            &objects,
            &store.path().join("objects"),
            &mut limits.storage_bytes.clone(),
            &mut 0,
        )?;
        // Check actual object hashes, not merely claimed filenames/types.
        run(
            store.path(),
            &["fsck", "--full", "--strict", "--no-reflogs"],
            limits,
        )?;
        Ok(Self {
            root,
            common,
            gitdir,
            store,
            format,
            git_version,
            repo_id: repo_id.into(),
            limits,
        })
    }
    /// Caller-assigned local label; remote URLs never authenticate this identity.
    pub fn repo_id(&self) -> &str {
        &self.repo_id
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn common_dir(&self) -> &Path {
        &self.common
    }
    /// Observed pinned executable version, not repository identity or authentication.
    pub fn git_version(&self) -> &str {
        &self.git_version
    }
    pub fn object_format(&self) -> &str {
        &self.format
    }
    pub(crate) fn run(&self, args: &[&str]) -> Result<Vec<u8>> {
        run(self.store.path(), args, self.limits)
    }
    pub(crate) fn oid(&self, oid: &str) -> Result<()> {
        let n = if self.format == "sha1" { 40 } else { 64 };
        if oid.len() != n
            || !oid
                .bytes()
                .all(|x| x.is_ascii_digit() || (b'a'..=b'f').contains(&x))
        {
            return Err(Diagnostic::InvalidObject);
        }
        Ok(())
    }
    pub fn commit(&self, oid: &str) -> Result<String> {
        self.object(oid, "commit")?;
        Ok(oid.into())
    }
    pub(crate) fn object(&self, oid: &str, kind: &str) -> Result<()> {
        self.oid(oid)?;
        let out = self
            .run(&["cat-file", "-t", oid])
            .map_err(|_| Diagnostic::InvalidObject)?;
        if out != format!("{kind}\n").as_bytes() {
            return Err(Diagnostic::InvalidObject);
        }
        Ok(())
    }
}
