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
        self.index_manifest_with_observer(|| {})
    }
    fn index_manifest_with_observer(&self, after_copy: impl FnOnce()) -> Result<Manifest> {
        // All observations share this Repository's private scratch index. Keep
        // copy, Git read and manifest parsing together; a poisoned observation
        // cannot establish cleanliness and must never be silently recovered.
        let _scratch = self
            .scratch_index
            .lock()
            .map_err(|_| Diagnostic::UnsafeStorage)?;
        let p = self.gitdir.join("index");
        let meta = fs::symlink_metadata(&p)?;
        if !meta.is_file() || meta.len() > self.limits.storage_bytes {
            return Err(Diagnostic::UnsafeStorage);
        }
        fs::copy(p, self.store.path().join("index"))?;
        after_copy();
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

#[cfg(test)]
mod scratch_index_tests {
    use super::*;
    use std::process::Command;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    };
    use std::time::Duration;

    fn git(root: &Path, args: &[&str]) -> String {
        let out = Command::new("/usr/bin/git")
            .arg("-C")
            .arg(root)
            .args([
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
            ])
            .args(args)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", root)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap().trim().to_owned()
    }
    fn fixture() -> (tempfile::TempDir, Repository, String) {
        let directory = tempfile::tempdir().unwrap();
        git(directory.path(), &["init", "-q", "-b", "main"]);
        fs::write(directory.path().join("a"), "fixed tracked bytes\n").unwrap();
        git(directory.path(), &["add", "a"]);
        git(directory.path(), &["commit", "-qm", "seed"]);
        let head = git(directory.path(), &["rev-parse", "HEAD"]);
        let repository = Repository::discover(directory.path(), "repo").unwrap();
        (directory, repository, head)
    }
    fn source_bytes(root: &Path) -> (Vec<u8>, Vec<u8>) {
        (
            fs::read(root.join(".git/index")).unwrap(),
            fs::read(root.join("a")).unwrap(),
        )
    }

    #[test]
    fn copied_scratch_index_is_exclusive_until_observation_finishes() {
        let (directory, repository, _) = fixture();
        let before = source_bytes(directory.path());
        let (copied_tx, copied_rx) = mpsc::channel();
        let (attempt_tx, attempt_rx) = mpsc::channel();
        let (second_copy_tx, second_copy_rx) = mpsc::channel();
        let overlap = AtomicBool::new(false);
        let (first, second) = std::thread::scope(|scope| {
            let repository_ref = &repository;
            let overlap_ref = &overlap;
            let first = scope.spawn(move || {
                repository_ref.index_manifest_with_observer(|| {
                    copied_tx.send(()).unwrap();
                    if attempt_rx.recv_timeout(Duration::from_secs(2)).is_ok() {
                        overlap_ref.store(
                            second_copy_rx
                                .recv_timeout(Duration::from_millis(200))
                                .is_ok(),
                            Ordering::SeqCst,
                        );
                    }
                })
            });
            let first_ready = copied_rx.recv_timeout(Duration::from_secs(2));
            let repository_ref = &repository;
            let second = scope.spawn(move || {
                attempt_tx.send(()).unwrap();
                repository_ref.index_manifest_with_observer(|| {
                    let _ = second_copy_tx.send(());
                })
            });
            // Join/release both workers before asserting the scheduling evidence.
            let joined = (first.join(), second.join());
            (first_ready, joined)
        });
        assert!(
            first.is_ok(),
            "first observation did not reach its copy checkpoint"
        );
        let (first, second) = second;
        let first = first.unwrap().unwrap();
        let second = second.unwrap().unwrap();
        assert!(
            !overlap.load(Ordering::SeqCst),
            "second observation overwrote shared scratch index before the first read completed"
        );
        assert_eq!(first, second);
        assert_eq!(source_bytes(directory.path()), before);
    }

    #[test]
    fn poisoned_scratch_index_fails_closed_without_writing_source() {
        let (directory, repository, _) = fixture();
        let before = source_bytes(directory.path());
        let panic = std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    let _guard = repository.scratch_index.lock().unwrap();
                    panic!("fixture interrupted while scratch observation was owned");
                })
                .join()
        });
        assert!(panic.is_err());
        assert!(matches!(
            repository.resolve_subject(SubjectRequest::Index),
            Err(Diagnostic::UnsafeStorage)
        ));
        assert_eq!(source_bytes(directory.path()), before);
    }

    #[test]
    fn real_concurrent_candidate_validation_has_no_false_binding_or_source_writes() {
        use crate::{candidate::CandidateRequest, scope::TaskScope};
        let (directory, repository, head) = fixture();
        let before = source_bytes(directory.path());
        let subject = repository
            .resolve_subject(SubjectRequest::Commit(head.clone()))
            .unwrap();
        let scope = TaskScope::advisory(
            "task",
            vec!["R".into()],
            vec![b"a".to_vec()],
            &"a".repeat(64),
            None,
        )
        .unwrap();
        let candidate = repository
            .prepare_candidate(
                &subject,
                &scope,
                &CandidateRequest {
                    worktree_id: "worktree".into(),
                    base_oid: head.clone(),
                    merge_group_id: None,
                    members: vec![],
                },
            )
            .unwrap();
        let failures = std::thread::scope(|scope| {
            (0..8)
                .map(|worker| {
                    let candidate = &candidate;
                    let repository = &repository;
                    scope.spawn(move || {
                        (0..32)
                            .filter_map(|iteration| {
                                candidate
                                    .validate(repository)
                                    .err()
                                    .map(|error| (worker, iteration, error))
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect::<Vec<_>>()
                .into_iter()
                .flat_map(|worker| worker.join().unwrap())
                .collect::<Vec<_>>()
        });
        assert!(
            failures.is_empty(),
            "stable source produced false validation failures: {failures:?}"
        );
        assert_eq!(source_bytes(directory.path()), before);
        assert_eq!(git(directory.path(), &["rev-parse", "HEAD"]), head);
    }
}
