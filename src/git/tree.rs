//! Bounded regular-file reads from the repository's isolated object database.
//! No checkout, filters, submodules, source worktree or caller-defined command.
use super::{repository::Repository, runner};
use crate::{Diagnostic, Result};
use std::{collections::BTreeMap, time::Instant};
#[derive(Debug)]
pub struct CommitFile {
    path: Vec<u8>,
    mode: String,
    object_oid: String,
    contents: Vec<u8>,
}
impl CommitFile {
    pub fn path(&self) -> &[u8] {
        &self.path
    }
    pub fn mode(&self) -> &str {
        &self.mode
    }
    pub fn object_oid(&self) -> &str {
        &self.object_oid
    }
    pub fn contents(&self) -> &[u8] {
        &self.contents
    }
}
impl Repository {
    /// Exact committed regular files in byte-path order. Limits cover the whole
    /// call, including type validation, tree listing and all blob reads.
    /// Repository discovery already fsck-verified the private copied objects;
    /// subsequent changes to source object files cannot affect these reads.
    /// Symlinks/gitlinks are explicitly unsupported, never followed or omitted.
    pub fn read_commit_files(&self, oid: &str) -> Result<Vec<CommitFile>> {
        self.oid(oid)?;
        let started = Instant::now();
        let run = |args: &[&str]| {
            let mut limits = self.limits;
            limits.timeout = limits
                .timeout
                .checked_sub(started.elapsed())
                .ok_or(Diagnostic::LimitExceeded)?;
            if limits.timeout.is_zero() {
                return Err(Diagnostic::LimitExceeded);
            }
            runner::run(self.store.path(), args, limits)
        };
        if run(&["cat-file", "-t", oid])? != b"commit\n" {
            return Err(Diagnostic::InvalidObject);
        }
        let raw = run(&["ls-tree", "-r", "-l", "-z", "--full-tree", oid])?;
        let mut metadata = BTreeMap::new();
        let mut left = self.limits.storage_bytes;
        for entry in raw.split(|b| *b == 0).filter(|e| !e.is_empty()) {
            if metadata.len() >= 10000 {
                return Err(Diagnostic::LimitExceeded);
            }
            let tab = entry
                .iter()
                .position(|b| *b == b'\t')
                .ok_or(Diagnostic::InvalidObject)?;
            let columns = std::str::from_utf8(&entry[..tab])
                .map_err(|_| Diagnostic::InvalidObject)?
                .split_whitespace()
                .collect::<Vec<_>>();
            if columns.len() != 4
                || !matches!(columns[0], "100644" | "100755")
                || columns[1] != "blob"
            {
                return Err(Diagnostic::UnsafeStorage);
            }
            self.oid(columns[2])?;
            let size = columns[3]
                .parse::<u64>()
                .map_err(|_| Diagnostic::InvalidObject)?;
            left = left.checked_sub(size).ok_or(Diagnostic::LimitExceeded)?;
            if size > self.limits.output_bytes as u64 {
                return Err(Diagnostic::LimitExceeded);
            }
            let path = entry[tab + 1..].to_vec();
            if !crate::scope::path_valid(&path) {
                return Err(Diagnostic::UnsafeStorage);
            }
            if metadata
                .insert(path, (columns[0].to_owned(), columns[2].to_owned(), size))
                .is_some()
            {
                return Err(Diagnostic::InvalidObject);
            }
        }
        let mut files = Vec::with_capacity(metadata.len());
        for (path, (mode, object_oid, size)) in metadata {
            let contents = run(&["cat-file", "blob", &object_oid])?;
            if contents.len() as u64 != size {
                return Err(Diagnostic::InvalidObject);
            }
            files.push(CommitFile {
                path,
                mode,
                object_oid,
                contents,
            });
        }
        Ok(files)
    }
}
