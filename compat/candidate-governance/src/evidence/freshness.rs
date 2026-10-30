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
        admit_policy(policy)?;
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

/// Count borrowed serialization before allocating or hashing attacker-sized input.
pub(crate) fn admit<T: serde::Serialize + ?Sized>(value: &T, limit: usize) -> Result<(), String> {
    struct Counter {
        remaining: usize,
    }
    impl std::io::Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.remaining = self
                .remaining
                .checked_sub(bytes.len())
                .ok_or_else(|| std::io::Error::other("budget"))?;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    serde_json::to_writer(Counter { remaining: limit }, value)
        .map_err(|_| "evidence budget exceeded".into())
}
pub(crate) fn admit_policy(policy: &EligibilityPolicy) -> Result<(), String> {
    admit(policy, 1024 * 1024)
}
pub(crate) fn admit_cause(cause: Option<&str>) -> Result<(), String> {
    if cause.is_some_and(|s| s.len() > 16 * 1024) {
        Err("cause budget exceeded".into())
    } else {
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FreshnessReason {
    Rechecked,
    InputsChanged,
    EvidenceChanged,
    InvalidEvidence,
}
/// Sanitized local-controller observation. This record grants no authority.
#[derive(Clone, Debug, serde::Serialize)]
pub struct FreshnessAudit {
    pub reason: FreshnessReason,
    pub expected_inputs: String,
    pub observed_inputs: Option<String>,
    pub expected_evidence: String,
    pub observed_evidence: Option<String>,
    pub cause_digest: Option<String>,
    pub timestamp: i64,
    pub evaluation: Option<guardengine::integration::eligibility::AuditRecord>,
}
pub struct FreshnessObservation {
    audit: FreshnessAudit,
    consumption: Option<super::consume::Consumption>,
}
impl FreshnessObservation {
    pub fn audit(&self) -> &FreshnessAudit {
        &self.audit
    }
    pub fn consumption(&self) -> Option<&super::consume::Consumption> {
        self.consumption.as_ref()
    }
}
/// Current dependency/configuration identities and time come from the protected controller.
#[derive(Clone, Copy)]
pub struct FreshnessContext<'a> {
    pub dependencies: &'a str,
    pub config: &'a str,
    pub now: i64,
    pub cause: Option<&'a str>,
}
/// Opaque process-local input snapshot, never a reusable eligibility result.
/// Each retry/new run needs a new session. Equal timestamps are allowed; rollback is not.
pub struct FreshnessSession {
    key: ReuseKey,
    evidence: String,
    last_time: i64,
}
impl FreshnessSession {
    pub fn freeze(
        repo: &Repository,
        candidate: &CandidateSnapshot,
        bundle: &super::envelope::CheckBundle,
        policy: &EligibilityPolicy,
        context: FreshnessContext<'_>,
    ) -> Result<Self, String> {
        if context.now < 0 {
            return Err("invalid controller time".into());
        }
        admit_cause(context.cause)?;
        admit(bundle, guardengine::integration::MAX_ARTIFACT_BYTES)?;
        let key = ReuseKey::new(
            repo,
            candidate,
            policy,
            context.dependencies,
            context.config,
        )?;
        let evidence = guardengine::digest_json(bundle).map_err(|_| "invalid evidence")?;
        Ok(Self {
            key,
            evidence,
            last_time: context.now,
        })
    }
    #[allow(clippy::too_many_arguments)]
    pub fn observe(
        &mut self,
        repo: &Repository,
        candidate: &CandidateSnapshot,
        bundle: &super::envelope::CheckBundle,
        policy: &EligibilityPolicy,
        provider: &dyn guardengine::integration::eligibility::AuthorityProvider,
        context: FreshnessContext<'_>,
    ) -> Result<FreshnessObservation, String> {
        if context.now < self.last_time {
            return Err("controller clock rollback".into());
        }
        // Advance before fallible admission: a rejected request cannot reopen an older clock.
        self.last_time = context.now;
        admit_cause(context.cause)?;
        let mut audit = FreshnessAudit {
            reason: FreshnessReason::InvalidEvidence,
            expected_inputs: self.key.digest().into(),
            observed_inputs: None,
            expected_evidence: self.evidence.clone(),
            observed_evidence: None,
            cause_digest: context
                .cause
                .map(|s| format!("sha256:{}", crate::subject::hash(s.as_bytes()))),
            timestamp: context.now,
            evaluation: None,
        };
        let mut consumption = None;
        if admit(bundle, guardengine::integration::MAX_ARTIFACT_BYTES).is_ok()
            && let Ok(key) = ReuseKey::new(
                repo,
                candidate,
                policy,
                context.dependencies,
                context.config,
            )
        {
            let digest = guardengine::digest_json(bundle).map_err(|_| "invalid evidence")?;
            audit.observed_inputs = Some(key.digest().into());
            audit.observed_evidence = Some(digest.clone());
            if !self.key.same_inputs(&key) {
                audit.reason = FreshnessReason::InputsChanged;
            } else if self.evidence != digest {
                audit.reason = FreshnessReason::EvidenceChanged;
            } else if let Ok(result) = super::consume::consume_bound(
                repo,
                candidate,
                bundle,
                policy,
                provider,
                super::consume::ConsumptionContext {
                    key: &key,
                    now: context.now,
                    cause: context.cause,
                },
            ) {
                audit.reason = FreshnessReason::Rechecked;
                audit.evaluation = Some(result.result().audit.clone());
                consumption = Some(result);
            }
        }
        Ok(FreshnessObservation { audit, consumption })
    }
}
