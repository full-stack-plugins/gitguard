//! Input identity only. Equal keys never cache producer/approval validity.
use crate::{Repository, candidate::CandidateSnapshot};
use guardengine::integration::{attempt_store::Target, eligibility::EligibilityPolicy};
#[derive(Clone)]
pub struct ReuseKey {
    pub(crate) target: Target,
    pub(crate) candidate_digest: String,
    pub(crate) policy_digest: String,
    digest: String,
}
impl ReuseKey {
    pub fn new(
        repo: &Repository,
        candidate: &CandidateSnapshot,
        policy: &EligibilityPolicy,
        dependencies: &str,
        config: &str,
    ) -> Result<Self, String> {
        candidate.validate(repo).map_err(|_| "invalid candidate")?;
        for digest in [dependencies, config] {
            if !digest
                .strip_prefix("sha256:")
                .is_some_and(crate::scope::digest_valid)
            {
                return Err("invalid dependency/configuration digest".into());
            }
        }
        let candidate_digest = candidate.binding_digest();
        let policy_digest = policy.content_digest();
        let digest = guardengine::digest_json(&(
            "gitguard.reuse/v1alpha1",
            &candidate_digest,
            &policy_digest,
            dependencies,
            config,
        ))
        .map_err(|_| "reuse serialization failed")?;
        Ok(Self {
            target: Target {
                repo_id: candidate.repo_id().into(),
                task_id: candidate.task_id().into(),
                requirement_ids: candidate.requirement_ids().to_vec(),
            },
            candidate_digest,
            policy_digest,
            digest,
        })
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn same_inputs(&self, other: &Self) -> bool {
        self.digest == other.digest
    }
}
