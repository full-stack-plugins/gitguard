use crate::{Diagnostic, Result};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskScope {
    pub(crate) task_id: String,
    pub(crate) requirement_ids: Vec<String>,
    pub(crate) allowed_paths: Vec<Vec<u8>>,
    pub(crate) policy_digest: String,
    #[serde(deserialize_with = "required_nullable")]
    pub(crate) baseline_digest: Option<String>,
}
pub(crate) fn digest_valid(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|x| x.is_ascii_digit() || (b'a'..=b'f').contains(&x))
}
pub(crate) fn path_valid(p: &[u8]) -> bool {
    !p.is_empty()
        && !p.contains(&0)
        && !p.contains(&b'\\')
        && p.split(|x| *x == b'/')
            .all(|x| !x.is_empty() && x != b"." && x != b".." && x != b".git")
}
impl TaskScope {
    pub fn advisory(
        task: &str,
        ids: Vec<String>,
        paths: Vec<Vec<u8>>,
        policy: &str,
        baseline: Option<String>,
    ) -> Result<Self> {
        let s = Self {
            task_id: task.into(),
            requirement_ids: ids,
            allowed_paths: paths,
            policy_digest: policy.into(),
            baseline_digest: baseline,
        };
        s.validate()?;
        Ok(s)
    }
    pub fn validate(&self) -> Result<()> {
        if self.task_id.is_empty()
            || self.requirement_ids.is_empty()
            || self.requirement_ids.iter().any(|x| x.is_empty())
            || self.requirement_ids.windows(2).any(|x| x[0] >= x[1])
            || !digest_valid(&self.policy_digest)
            || self.baseline_digest.is_some()
            || self.allowed_paths.iter().any(|p| !path_valid(p))
        {
            return Err(Diagnostic::InvalidScope);
        }
        Ok(())
    }
    pub fn requirement_ids(&self) -> &[String] {
        &self.requirement_ids
    }
    pub(crate) fn permits(&self, path: &[u8]) -> bool {
        path_valid(path)
            && self
                .allowed_paths
                .iter()
                .any(|p| path == p || path.starts_with(&[p.as_slice(), b"/"].concat()))
    }
}

fn required_nullable<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Option<String>, D::Error> {
    Option::<String>::deserialize(d)
}

mod protected;
pub use protected::{ImmutableScopeSource, ProtectedScopeRequest, ProtectedTaskScope};
