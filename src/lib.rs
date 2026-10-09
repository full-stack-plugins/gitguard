//! Advisory, read-only Git candidate observation.
pub mod git;
pub use git::repository::Repository;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Diagnostic {
    InvalidRepository,
    UnsupportedFormat,
    UnsafeStorage,
    LimitExceeded,
    GitFailed,
    UnsupportedProcessProfile,
    ProcessCleanupFailed,
    ProcessPolicyDenied,
    InvalidObject,
    InvalidScope,
    DirtySubject,
    InvalidBinding,
    Conflict,
    Cancelled,
}
pub type Result<T> = std::result::Result<T, Diagnostic>;
impl From<std::io::Error> for Diagnostic {
    fn from(_: std::io::Error) -> Self {
        Self::InvalidRepository
    }
}
pub mod candidate;
pub mod cli;
pub mod conflicts;
pub mod evidence;
pub mod execution;
pub mod preflight;
pub mod scope;
pub mod subject;
pub mod worktree;
