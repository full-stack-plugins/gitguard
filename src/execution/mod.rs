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
/// Historical generic entrypoint: never routes to a mutation transport or credential provider.
pub fn request(config: &ExecutionConfig, _: Operation) -> Result<(), ExecutionError> {
    if !cfg!(feature = "privileged-execution") || !config.enabled {
        return Err(ExecutionError::Disabled);
    }
    Err(ExecutionError::AuthorizationUnavailable)
}

pub mod grant;

#[cfg(target_os = "linux")]
pub mod intent;

#[cfg(target_os = "linux")]
pub mod apply;
#[cfg(target_os = "linux")]
#[path = "../../adapters/platform_write.rs"]
pub mod platform_write;
