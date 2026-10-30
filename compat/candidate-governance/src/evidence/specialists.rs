//! Read-only specialist eligibility with controller-frozen expectations.
//!
//! Policies, bytes and authority providers are supplied by the protected controller.
//! This module performs no URI resolution, network access, approval issuance or grant creation.
use crate::{Repository, candidate::CandidateSnapshot};
use guardengine::integration::{
    ArtifactRef, EvidenceProfile, GuardRunEnvelope, MAX_ARTIFACT_BYTES, MAX_ENVELOPE_BYTES,
    RunBinding,
    eligibility::{self, AuthorityProvider, EligibilityCode, EligibilityPolicy, EligibilityResult},
};
use std::collections::{BTreeMap, BTreeSet};

const MAX_SPECIALISTS: usize = 32;
const MAX_TOTAL_BYTES: usize = 64 * 1024 * 1024;

/// A separately authenticated baseline prerequisite. The reference is never dereferenced here.
/// Its policy is independent of technical review approval policy, but binds the same domain.
pub struct BaselineApproval {
    pub reference: String,
    pub policy: EligibilityPolicy,
    pub purpose: String,
}
pub struct RequiredSpecialist {
    pub id: String,
    pub policy: EligibilityPolicy,
    pub profile: EvidenceProfile,
    pub baseline: Option<BaselineApproval>,
}
/// Owned immutable bytes indexed by the complete reference, including media type.
/// URI strings are identifiers only; no filesystem or network resolver is installed.
pub struct ImmutableArtifactBytes {
    entries: BTreeMap<String, (ArtifactRef, Vec<u8>)>,
}
impl ImmutableArtifactBytes {
    pub fn new(artifacts: Vec<(ArtifactRef, Vec<u8>)>) -> Result<Self, String> {
        if artifacts.len() > MAX_SPECIALISTS * 3 {
            return Err("artifact count limit".into());
        }
        let mut total = 0usize;
        let mut entries = BTreeMap::new();
        for (reference, bytes) in artifacts {
            super::freshness::admit(&reference, 16 * 1024)?;
            total = total.checked_add(bytes.len()).ok_or("artifact budget")?;
            if bytes.len() > MAX_ARTIFACT_BYTES
                || total > MAX_TOTAL_BYTES
                || reference.uri.is_empty()
                || reference.media_type != "application/json"
                || reference.digest != format!("sha256:{}", crate::subject::hash(&bytes))
            {
                return Err("invalid immutable artifact".into());
            }
            if entries
                .insert(reference.uri.clone(), (reference, bytes))
                .is_some()
            {
                return Err("duplicate artifact URI".into());
            }
        }
        Ok(Self { entries })
    }
    fn resolve(&self, reference: Option<&ArtifactRef>) -> Result<&[u8], String> {
        let Some(reference) = reference else {
            return Ok(&[]);
        };
        let (expected, bytes) = self
            .entries
            .get(&reference.uri)
            .ok_or("missing artifact bytes")?;
        if expected != reference {
            return Err("artifact reference changed".into());
        }
        Ok(bytes)
    }
}
/// Frozen required set cannot be replaced or reduced at consumption time.
pub struct PreparedSpecialistConsumption {
    candidate_digest: String,
    required: BTreeMap<String, RequiredSpecialist>,
}
pub struct SpecialistConsumption {
    results: BTreeMap<String, EligibilityResult>,
}
impl SpecialistConsumption {
    pub fn eligible(&self) -> bool {
        self.results.values().all(|result| result.eligible)
    }
    pub fn results(&self) -> &BTreeMap<String, EligibilityResult> {
        &self.results
    }
}
fn common_binding(binding: &RunBinding, candidate: &CandidateSnapshot) -> bool {
    binding.repo_id == candidate.repo_id()
        && binding.task_id == candidate.task_id()
        && binding.worktree_id == candidate.worktree_id()
        && binding.requirement_ids == candidate.requirement_ids()
        && binding.candidate_oid == candidate.candidate_oid()
        && binding.base_oid == candidate.base_oid()
        && binding.merge_group_id.as_deref() == candidate.merge_group_id()
}
impl PreparedSpecialistConsumption {
    pub fn freeze(
        repo: &Repository,
        candidate: &CandidateSnapshot,
        required: Vec<RequiredSpecialist>,
    ) -> Result<Self, String> {
        candidate.validate(repo).map_err(|_| "invalid candidate")?;
        if required.is_empty() || required.len() > MAX_SPECIALISTS {
            return Err("invalid required specialist count".into());
        }
        let mut frozen = BTreeMap::new();
        let mut producers = BTreeSet::new();
        let mut action = None;
        for item in required {
            super::freshness::admit_policy(&item.policy)?;
            if item.id.trim().is_empty()
                || item.id.len() > 256
                || item.profile != EvidenceProfile::EngineBacked
                || !common_binding(&item.policy.binding, candidate)
                || !producers.insert(
                    guardengine::digest_json(&item.policy.producer)
                        .map_err(|_| "invalid producer")?,
                )
            {
                return Err("invalid specialist expectation".into());
            }
            if action.as_ref().is_some_and(|a| a != &item.policy.action) {
                return Err("specialist action differs".into());
            }
            action = Some(item.policy.action.clone());
            // A domain baseline is not the Git candidate's baseline. Require an explicit,
            // independently frozen authentication contract for every non-null domain baseline.
            match (&item.policy.binding.baseline_digest, &item.baseline) {
                (None, None) => {}
                (Some(_), Some(baseline)) => {
                    super::freshness::admit_policy(&baseline.policy)?;
                    if baseline.reference.trim().is_empty()
                        || baseline.reference.len() > 4096
                        || baseline.purpose.trim().is_empty()
                        || baseline.purpose.len() > 256
                        || baseline.policy.binding != item.policy.binding
                        || baseline.policy.contract_digest != item.policy.contract_digest
                        || baseline.policy.producer != item.policy.producer
                        || baseline.policy.action != item.policy.action
                        || !baseline
                            .policy
                            .approval_principals
                            .contains_key(&baseline.purpose)
                    {
                        return Err("invalid baseline authority expectation".into());
                    }
                }
                _ => return Err("baseline authentication requirement missing or extraneous".into()),
            }
            if frozen.insert(item.id.clone(), item).is_some() {
                return Err("duplicate required specialist".into());
            }
        }
        Ok(Self {
            candidate_digest: candidate.binding_digest(),
            required: frozen,
        })
    }
    /// Recheck both producer and approval authority on every read; no validity cache.
    #[allow(clippy::too_many_arguments)]
    pub fn consume(
        &self,
        repo: &Repository,
        candidate: &CandidateSnapshot,
        envelopes: &BTreeMap<String, GuardRunEnvelope>,
        bytes: &ImmutableArtifactBytes,
        authority: &dyn AuthorityProvider,
        now: i64,
        cause: Option<&str>,
    ) -> Result<SpecialistConsumption, String> {
        if now < 0 {
            return Err("invalid controller time".into());
        }
        super::freshness::admit_cause(cause)?;
        candidate.validate(repo).map_err(|_| "invalid candidate")?;
        if candidate.binding_digest() != self.candidate_digest
            || envelopes.keys().ne(self.required.keys())
        {
            return Err("candidate or required specialist set changed".into());
        }
        let mut results = BTreeMap::new();
        for (id, item) in &self.required {
            let envelope = &envelopes[id];
            super::freshness::admit(envelope, MAX_ENVELOPE_BYTES)?;
            let mut result = eligibility::evaluate_eligibility(
                envelope,
                eligibility::ArtifactBytes {
                    contract: bytes.resolve(envelope.artifacts.contract.as_ref())?,
                    facts: bytes.resolve(envelope.artifacts.facts.as_ref())?,
                    report: bytes.resolve(envelope.artifacts.report.as_ref())?,
                },
                &item.policy,
                authority,
                now,
                cause,
            );
            if let Some(baseline) = &item.baseline {
                let validation = authority
                    .verify_approval(&baseline.reference)
                    .map_err(|e| match e {
                        eligibility::AuthorityError::Unavailable => {
                            EligibilityCode::ProviderUnavailable
                        }
                        eligibility::AuthorityError::Untrusted => EligibilityCode::InvalidApproval,
                    })
                    .and_then(|record| {
                        eligibility::validate_approval_record(
                            &record,
                            &baseline.policy,
                            &baseline.purpose,
                            now,
                        )
                    });
                // Never replace a technical failure with successful approval, nor mutate reports.
                if result.eligible
                    && let Err(code) = validation
                {
                    result.eligible = false;
                    result.code = code;
                    result.audit.code = code;
                }
            }
            results.insert(id.clone(), result);
        }
        Ok(SpecialistConsumption { results })
    }
}
