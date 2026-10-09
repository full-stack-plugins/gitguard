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
        let config = std::str::from_utf8(&config).map_err(|_| Diagnostic::InvalidRepository)?;
        let mut format = "sha1".to_string();
        for line in config.lines() {
            if let Some((key, value)) = line.split_once('=')
                && key.trim().eq_ignore_ascii_case("objectformat")
            {
                format = value.trim().to_ascii_lowercase();
            }
        }
        if format != "sha1" && format != "sha256" {
            return Err(Diagnostic::UnsupportedFormat);
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
        let store = tempfile::tempdir()?;
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
            repo_id: repo_id.into(),
            limits,
        })
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn common_dir(&self) -> &Path {
        &self.common
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
