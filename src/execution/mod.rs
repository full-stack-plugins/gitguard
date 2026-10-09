use serde::{Deserialize, Serialize};
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionConfig {
    pub enabled: bool,
}
#[derive(Clone, Copy, Debug)]
pub enum Operation {
    CreateWorktree,
    CreateBranch,
    Push,
    Merge,
}
#[derive(Debug, PartialEq, Eq)]
pub enum ExecutionError {
    Disabled,
    AuthorizationUnavailable,
}
/// Deliberate security boundary: no mutation transport or credential provider is linked.
pub fn request(config: &ExecutionConfig, _: Operation) -> Result<(), ExecutionError> {
    if !cfg!(feature = "privileged-execution") || !config.enabled {
        return Err(ExecutionError::Disabled);
    }
    Err(ExecutionError::AuthorizationUnavailable)
}

pub mod grant;

#[cfg(target_os = "linux")]
pub mod intent;
