//! Independent detached regular-file worktree; never edits shared Git worktree metadata.
use super::{
    apply::{ApplyContext, ApplyError},
    grant::{GrantAction, GrantSession},
    intent::IntentStore,
    platform_write::ProtectedBareTarget,
    reconcile::{RecoveryError, RecoveryState, reconcile_identity},
};
use crate::git::runner::{Limits, run};
use std::{
    ffi::{CString, OsStr},
    fs::{File, OpenOptions},
    io::Write,
    os::{
        fd::AsRawFd,
        unix::{
            ffi::OsStrExt,
            fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
        },
    },
    path::{Path, PathBuf},
    sync::atomic::Ordering,
};

/// A controller-owned private publication directory, not an identity or grant provider.
pub struct ProtectedWorktreeRoot {
    directory: File,
    root: PathBuf,
    repo_id: String,
    identity: String,
}
impl ProtectedWorktreeRoot {
    pub fn open(root: &Path, repo_id: &str) -> Result<Self, ApplyError> {
        if repo_id.is_empty()
            || repo_id.len() > 128
            || !repo_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        {
            return Err(ApplyError::Binding);
        }
        let directory = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(root)
            .map_err(|_| ApplyError::Target)?;
        let mut stat = std::mem::MaybeUninit::<libc::statfs>::uninit();
        if unsafe { libc::fstatfs(directory.as_raw_fd(), stat.as_mut_ptr()) } != 0
            || ![0xef53, 0x58465342, 0x9123683e, 0x794c7630, 0x01021994]
                .contains(&unsafe { stat.assume_init() }.f_type)
        {
            return Err(ApplyError::Unsupported);
        }
        let metadata = directory.metadata().map_err(|_| ApplyError::Target)?;
        let identity = guardengine::digest_json(&(
            "gitguard.local-worktree-root/v1alpha1",
            repo_id,
            metadata.dev(),
            metadata.ino(),
        ))
        .map_err(|_| ApplyError::Binding)?;
        let target = Self {
            directory,
            root: std::fs::canonicalize(root).map_err(|_| ApplyError::Target)?,
            repo_id: repo_id.into(),
            identity,
        };
        target.unchanged()?;
        Ok(target)
    }
    fn path(&self) -> PathBuf {
        PathBuf::from(format!(
            "/proc/{}/fd/{}",
            std::process::id(),
            self.directory.as_raw_fd()
        ))
    }
    fn unchanged(&self) -> Result<(), ApplyError> {
        let metadata = self.directory.metadata().map_err(|_| ApplyError::Target)?;
        let current = std::fs::symlink_metadata(&self.root).map_err(|_| ApplyError::Target)?;
        if metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o077 != 0
            || !current.is_dir()
            || current.dev() != metadata.dev()
            || current.ino() != metadata.ino()
        {
            return Err(ApplyError::Target);
        }
        Ok(())
    }
    fn preflight(&self, context: &ApplyContext<'_>) -> Result<(), ApplyError> {
        self.unchanged()?;
        let expected = context.session.expectation();
        let name = &expected.request().target;
        // One direct child only. The frozen grant already validates its portable basename.
        if name.contains('/')
            || expected.repo_id() != self.repo_id
            || context.repository.repo_id() != self.repo_id
        {
            return Err(ApplyError::Binding);
        }
        match std::fs::symlink_metadata(self.path().join(name)) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            _ => Err(ApplyError::Target),
        }
    }
    pub fn reconcile(
        &self,
        session: &GrantSession,
        intents: &IntentStore,
    ) -> Result<RecoveryState, RecoveryError> {
        self.unchanged().map_err(|_| RecoveryError::Target)?;
        reconcile_identity(session, intents, &self.identity)
    }
}

pub struct WorktreeReceipt {
    candidate_oid: String,
    operation_id: String,
    request_digest: String,
    name: String,
}
impl WorktreeReceipt {
    pub fn candidate_oid(&self) -> &str {
        &self.candidate_oid
    }
    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }
    pub fn request_digest(&self) -> &str {
        &self.request_digest
    }
    pub fn name(&self) -> &str {
        &self.name
    }
}

pub fn apply_worktree(
    context: &ApplyContext<'_>,
    target: &ProtectedWorktreeRoot,
) -> Result<WorktreeReceipt, ApplyError> {
    if !cfg!(feature = "privileged-execution") || !context.config.enabled {
        return Err(ApplyError::Disabled);
    }
    let expected = context.session.expectation();
    if expected.request().action != GrantAction::CreateWorktree {
        return Err(ApplyError::Unsupported);
    }
    if context.cancel.load(Ordering::SeqCst) {
        return Err(ApplyError::Cancelled);
    }
    crate::evidence::freshness::admit(context.candidate, 1024 * 1024)
        .map_err(|_| ApplyError::Binding)?;
    context
        .candidate
        .validate(context.repository)
        .map_err(|_| ApplyError::Binding)?;
    if expected.binding_digest() != context.candidate.binding_digest()
        || expected.candidate_oid() != context.candidate.candidate_oid()
    {
        return Err(ApplyError::Binding);
    }
    target.preflight(context)?;
    // Bounded reader rejects symlinks/gitlinks/path escapes before any durable claim.
    let files = context
        .repository
        .read_commit_files(expected.candidate_oid())
        .map_err(|_| ApplyError::Unsupported)?;
    context
        .session
        .validate(context.grant_reference, context.authority, context.clock)
        .map_err(|_| ApplyError::Authority)?;
    let prepared = context
        .intents
        .prepare_for_platform(expected, &target.identity)
        .map_err(|_| ApplyError::Intent)?;
    if context.cancel.load(Ordering::SeqCst) {
        return Err(ApplyError::Cancelled);
    }
    let reservation = context
        .intents
        .claim(&prepared)
        .map_err(|_| ApplyError::Intent)?;
    let result = (|| {
        context
            .session
            .validate(context.grant_reference, context.authority, context.clock)
            .map_err(|_| ApplyError::Authority)?;
        target.preflight(context)?;
        if context.cancel.load(Ordering::SeqCst) {
            return Err(ApplyError::Cancelled);
        }
        // Keep attempt-owned staging on every post-claim failure; never remove/replay unknown work.
        let staging = tempfile::Builder::new()
            .prefix(".gitguard-staging-")
            .permissions(std::fs::Permissions::from_mode(0o700))
            .tempdir_in(target.path())
            .map_err(|_| ApplyError::Target)?
            .keep();
        let gitdir = staging.join(".git");
        std::fs::create_dir(&gitdir).map_err(|_| ApplyError::Target)?;
        std::fs::set_permissions(&gitdir, std::fs::Permissions::from_mode(0o700))
            .map_err(|_| ApplyError::Target)?;
        run(
            &gitdir,
            &[
                "init",
                "--bare",
                "--quiet",
                &format!("--object-format={}", context.repository.object_format()),
            ],
            Limits::default(),
        )
        .map_err(|_| ApplyError::Target)?;
        ProtectedBareTarget::open(&gitdir, &target.repo_id)
            .map_err(|_| ApplyError::Target)?
            .transfer_candidate(context.repository, expected)
            .map_err(|_| ApplyError::Target)?;
        run(
            &gitdir,
            &["config", "core.bare", "false"],
            Limits::default(),
        )
        .map_err(|_| ApplyError::Target)?;
        // This is a new unpublished private repository: install the exact detached HEAD,
        // without invoking reflog identity resolution or a helper process.
        std::fs::write(
            gitdir.join("HEAD"),
            format!("{}\n", expected.candidate_oid()),
        )
        .map_err(|_| ApplyError::Target)?;
        run(
            &gitdir,
            &["read-tree", expected.candidate_oid()],
            Limits::default(),
        )
        .map_err(|_| ApplyError::Target)?;
        for file in files {
            let path = staging.join(OsStr::from_bytes(file.path()));
            std::fs::create_dir_all(path.parent().ok_or(ApplyError::Binding)?)
                .map_err(|_| ApplyError::Target)?;
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(if file.mode() == "100755" {
                    0o755
                } else {
                    0o644
                })
                .open(&path)
                .map_err(|_| ApplyError::Target)?;
            output
                .write_all(file.contents())
                .map_err(|_| ApplyError::Target)?;
            // create mode honors umask; restore the exact Git executable-bit semantics.
            output
                .set_permissions(std::fs::Permissions::from_mode(
                    if file.mode() == "100755" {
                        0o755
                    } else {
                        0o644
                    },
                ))
                .map_err(|_| ApplyError::Target)?;
            output.sync_all().map_err(|_| ApplyError::Target)?;
        }
        // Sync every generated regular file and directory (bounded by admitted source/target data).
        sync_tree(&staging)?;
        context
            .session
            .validate(context.grant_reference, context.authority, context.clock)
            .map_err(|_| ApplyError::Authority)?;
        target.preflight(context)?;
        if context.cancel.load(Ordering::SeqCst) {
            return Err(ApplyError::Cancelled);
        }
        let from = CString::new(staging.file_name().ok_or(ApplyError::Target)?.as_bytes())
            .map_err(|_| ApplyError::Target)?;
        let to =
            CString::new(expected.request().target.as_bytes()).map_err(|_| ApplyError::Target)?;
        if unsafe {
            libc::renameat2(
                target.directory.as_raw_fd(),
                from.as_ptr(),
                target.directory.as_raw_fd(),
                to.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        } != 0
        {
            return Err(ApplyError::Target);
        }
        target
            .directory
            .sync_all()
            .map_err(|_| ApplyError::Target)?;
        if context.cancel.load(Ordering::SeqCst) {
            return Err(ApplyError::Cancelled);
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = context.intents.mark_uncertain(reservation);
        return Err(ApplyError::RecoveryRequired);
    }
    if context.intents.mark_applied(&reservation).is_err() {
        let _ = context.intents.mark_uncertain(reservation);
        return Err(ApplyError::RecoveryRequired);
    }
    Ok(WorktreeReceipt {
        candidate_oid: expected.candidate_oid().into(),
        operation_id: expected.operation_id().into(),
        request_digest: prepared.request_digest().into(),
        name: expected.request().target.clone(),
    })
}
fn sync_tree(root: &Path) -> Result<(), ApplyError> {
    let mut pending = vec![root.to_path_buf()];
    let mut directories = Vec::new();
    let mut entries = 0;
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).map_err(|_| ApplyError::Target)? {
            let entry = entry.map_err(|_| ApplyError::Target)?;
            entries += 1;
            if entries > 20000 {
                return Err(ApplyError::Target);
            }
            if entry.file_type().map_err(|_| ApplyError::Target)?.is_dir() {
                pending.push(entry.path());
            } else {
                File::open(entry.path())
                    .and_then(|file| file.sync_all())
                    .map_err(|_| ApplyError::Target)?;
            }
        }
        directories.push(directory);
    }
    for directory in directories.into_iter().rev() {
        File::open(directory)
            .and_then(|file| file.sync_all())
            .map_err(|_| ApplyError::Target)?;
    }
    Ok(())
}
