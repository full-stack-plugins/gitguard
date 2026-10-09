use crate::{Diagnostic, Repository, Result};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::{ffi::OsStrExt, fs::PermissionsExt},
    path::Path,
};
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubjectRequest {
    Commit(String),
    TreePreview(String),
    Index,
    Worktree,
}
#[derive(Debug, Clone)]
pub struct FrozenSubject {
    pub(crate) request: SubjectRequest,
    pub(crate) digest: String,
    pub(crate) clean: bool,
    pub(crate) repo_id: String,
}
impl FrozenSubject {
    pub fn clean(&self) -> bool {
        self.clean
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn candidate_oid(&self) -> Option<&str> {
        match &self.request {
            SubjectRequest::Commit(oid) => Some(oid),
            _ => None,
        }
    }
}
type Manifest = BTreeMap<Vec<u8>, (String, String)>;
pub(crate) fn hash(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}
fn digest(m: &Manifest) -> String {
    let mut h = Sha256::new();
    for (p, (mode, bytes)) in m {
        h.update((p.len() as u64).to_be_bytes());
        h.update(p);
        h.update(mode);
        h.update(bytes);
    }
    format!("{:x}", h.finalize())
}
impl Repository {
    fn object_manifest(&self, raw: &[u8], index: bool) -> Result<Manifest> {
        let mut m = Manifest::new();
        for entry in raw.split(|x| *x == 0).filter(|x| !x.is_empty()) {
            let tab = entry
                .iter()
                .position(|x| *x == b'\t')
                .ok_or(Diagnostic::GitFailed)?;
            let cols = std::str::from_utf8(&entry[..tab])
                .map_err(|_| Diagnostic::GitFailed)?
                .split_whitespace()
                .collect::<Vec<_>>();
            if cols.len() != 3 {
                return Err(Diagnostic::GitFailed);
            }
            let mode = cols[0];
            let oid = if index {
                if cols[2] != "0" {
                    return Err(Diagnostic::Conflict);
                }
                cols[1]
            } else {
                cols[2]
            };
            self.oid(oid)?;
            let path = entry[tab + 1..].to_vec();
            if !crate::scope::path_valid(&path) {
                return Err(Diagnostic::UnsafeStorage);
            }
            let content = if mode == "160000" {
                hash(oid.as_bytes())
            } else {
                hash(&self.run(&["cat-file", "blob", oid])?)
            };
            if m.insert(path, (mode.into(), content)).is_some() {
                return Err(Diagnostic::InvalidObject);
            }
        }
        Ok(m)
    }
    fn worktree_manifest(&self) -> Result<Manifest> {
        fn walk(
            root: &Path,
            dir: &Path,
            out: &mut Manifest,
            left: &mut u64,
            count: &mut usize,
        ) -> Result<()> {
            for e in fs::read_dir(dir)? {
                let e = e?;
                if dir == root && e.file_name() == ".git" {
                    continue;
                }
                *count += 1;
                if *count > 100000 {
                    return Err(Diagnostic::LimitExceeded);
                }
                let p = e.path();
                let t = fs::symlink_metadata(&p)?;
                let path = p
                    .strip_prefix(root)
                    .map_err(|_| Diagnostic::UnsafeStorage)?
                    .as_os_str()
                    .as_bytes()
                    .to_vec();
                if !crate::scope::path_valid(&path) {
                    return Err(Diagnostic::UnsafeStorage);
                }
                if t.is_dir() {
                    walk(root, &p, out, left, count)?
                } else {
                    let (mode, data) = if t.file_type().is_symlink() {
                        ("120000", fs::read_link(&p)?.as_os_str().as_bytes().to_vec())
                    } else if t.is_file() {
                        *left = left.checked_sub(t.len()).ok_or(Diagnostic::LimitExceeded)?;
                        let data = fs::read(&p)?;
                        if data.len() as u64 != t.len() {
                            return Err(Diagnostic::UnsafeStorage);
                        }
                        (
                            if t.permissions().mode() & 0o111 != 0 {
                                "100755"
                            } else {
                                "100644"
                            },
                            data,
                        )
                    } else {
                        return Err(Diagnostic::UnsafeStorage);
                    };
                    out.insert(path, (mode.into(), hash(&data)));
                }
            }
            Ok(())
        }
        let mut m = Manifest::new();
        walk(
            self.root(),
            self.root(),
            &mut m,
            &mut self.limits.storage_bytes.clone(),
            &mut 0,
        )?;
        Ok(m)
    }
    fn index_manifest(&self) -> Result<Manifest> {
        let p = self.gitdir.join("index");
        let meta = fs::symlink_metadata(&p)?;
        if !meta.is_file() || meta.len() > self.limits.storage_bytes {
            return Err(Diagnostic::UnsafeStorage);
        }
        fs::copy(p, self.store.path().join("index"))?;
        self.object_manifest(&self.run(&["ls-files", "--stage", "-z"])?, true)
    }
    pub fn resolve_subject(&self, request: SubjectRequest) -> Result<FrozenSubject> {
        let actual = self.worktree_manifest()?;
        let manifest = match &request {
            SubjectRequest::Commit(oid) => {
                self.commit(oid)?;
                self.object_manifest(
                    &self.run(&["ls-tree", "-r", "-z", "--full-tree", oid])?,
                    false,
                )?
            }
            SubjectRequest::TreePreview(oid) => {
                self.object(oid, "tree")?;
                self.object_manifest(
                    &self.run(&["ls-tree", "-r", "-z", "--full-tree", oid])?,
                    false,
                )?
            }
            SubjectRequest::Worktree => actual.clone(),
            SubjectRequest::Index => self.index_manifest()?,
        };
        let clean = manifest == actual
            && (!matches!(request, SubjectRequest::Commit(_))
                || self.index_manifest()? == manifest);
        Ok(FrozenSubject {
            request,
            digest: digest(&manifest),
            clean,
            repo_id: self.repo_id.clone(),
        })
    }
    /// Re-observes source bytes; an old clean snapshot does not prove current cleanliness.
    pub fn verify_source(&self, subject: &FrozenSubject) -> Result<()> {
        if subject.repo_id != self.repo_id {
            return Err(Diagnostic::InvalidBinding);
        }
        let current = self.resolve_subject(subject.request.clone())?;
        if !current.clean || current.digest != subject.digest {
            return Err(Diagnostic::DirtySubject);
        }
        Ok(())
    }
}
