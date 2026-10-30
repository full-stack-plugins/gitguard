//! In-memory, process-local leases. Cancellation never removes a directory or a Git ref.
use crate::{Diagnostic, Repository, Result, scope::TaskScope, subject::hash};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeBinding {
    id: String,
    root: PathBuf,
    repo_id: String,
    task_id: String,
    requirement_ids: Vec<String>,
}
impl WorktreeBinding {
    pub fn id(&self) -> &str {
        &self.id
    }
}
struct Entry {
    binding: WorktreeBinding,
    expires: Instant,
    output: Option<String>,
}
pub struct Registry {
    session: tempfile::TempDir,
    next: AtomicU64,
    entries: Mutex<HashMap<PathBuf, Entry>>,
}
impl Registry {
    pub fn new() -> Result<Self> {
        Ok(Self {
            session: tempfile::tempdir()?,
            next: AtomicU64::new(0),
            entries: Mutex::new(HashMap::new()),
        })
    }
    pub fn register(
        &self,
        repo: &Repository,
        scope: &TaskScope,
        lease: Duration,
    ) -> Result<WorktreeBinding> {
        scope.validate()?;
        let expires = Instant::now()
            .checked_add(lease)
            .ok_or(Diagnostic::InvalidBinding)?;
        if lease.is_zero() {
            return Err(Diagnostic::InvalidBinding);
        }
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| Diagnostic::InvalidBinding)?;
        if entries
            .get(repo.root())
            .is_some_and(|e| e.expires > Instant::now())
        {
            return Err(Diagnostic::InvalidBinding);
        }
        let id = hash(
            format!(
                "{}:{}",
                self.session.path().display(),
                self.next.fetch_add(1, Ordering::Relaxed)
            )
            .as_bytes(),
        );
        let binding = WorktreeBinding {
            id,
            root: repo.root().into(),
            repo_id: repo.repo_id.clone(),
            task_id: scope.task_id.clone(),
            requirement_ids: scope.requirement_ids.clone(),
        };
        entries.insert(
            binding.root.clone(),
            Entry {
                binding: binding.clone(),
                expires,
                output: None,
            },
        );
        Ok(binding)
    }
    pub fn cancel(&self, b: &WorktreeBinding) -> Result<()> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| Diagnostic::InvalidBinding)?;
        if !entries.get(&b.root).is_some_and(|e| e.binding == *b) {
            return Err(Diagnostic::Cancelled);
        }
        entries.remove(&b.root);
        Ok(())
    }
    pub fn complete(&self, b: &WorktreeBinding, output: String) -> Result<()> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| Diagnostic::InvalidBinding)?;
        let e = entries.get_mut(&b.root).ok_or(Diagnostic::Cancelled)?;
        if e.binding != *b || e.expires <= Instant::now() || e.output.is_some() {
            return Err(Diagnostic::Cancelled);
        }
        e.output = Some(output);
        Ok(())
    }
    pub fn output(&self, b: &WorktreeBinding) -> Result<Option<String>> {
        let entries = self
            .entries
            .lock()
            .map_err(|_| Diagnostic::InvalidBinding)?;
        let e = entries.get(&b.root).ok_or(Diagnostic::Cancelled)?;
        if e.binding != *b || e.expires <= Instant::now() {
            return Err(Diagnostic::Cancelled);
        }
        Ok(e.output.clone())
    }
}
