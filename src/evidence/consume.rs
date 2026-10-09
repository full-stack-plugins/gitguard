//! Local GitGuard bundle consumption through real GE trust ports; no provider is installed.
use crate::{
    Repository,
    candidate::CandidateSnapshot,
    evidence::envelope::{CHECK_VERSION, CheckBundle},
    subject::hash,
};
use guardengine::integration::{
    self, GuardRunEnvelope,
    eligibility::{
        ApprovalRecord, ArtifactBytes, AuthorityError, AuthorityProvider, EligibilityPolicy,
        EligibilityResult, ProducerRecord, evaluate_eligibility,
    },
};
pub struct Consumption {
    pub(crate) result: EligibilityResult,
    pub(crate) candidate_digest: String,
    pub(crate) policy_digest: String,
    pub(crate) run_id: String,
}
impl Consumption {
    pub fn result(&self) -> &EligibilityResult {
        &self.result
    }
}
/// Safe default: artifact recomputation alone never authenticates an issuer.
pub struct UnavailableAuthority;
impl AuthorityProvider for UnavailableAuthority {
    fn verify_producer(
        &self,
        _: &GuardRunEnvelope,
        _: &str,
    ) -> Result<ProducerRecord, AuthorityError> {
        Err(AuthorityError::Unavailable)
    }
    fn verify_approval(&self, _: &str) -> Result<ApprovalRecord, AuthorityError> {
        Err(AuthorityError::Unavailable)
    }
}
pub fn consume(
    repo: &Repository,
    candidate: &CandidateSnapshot,
    bundle: &CheckBundle,
    policy: &EligibilityPolicy,
    provider: &dyn AuthorityProvider,
    now: i64,
    cause: Option<&str>,
) -> Result<Consumption, String> {
    if bundle.api_version != CHECK_VERSION {
        return Err("unsupported bundle version".into());
    }
    candidate
        .validate(repo)
        .map_err(|_| "candidate observation invalid")?;
    let envelope = &bundle.envelope;
    envelope
        .validate(integration::EvidenceProfile::EngineBacked)
        .map_err(|_| "invalid envelope")?;
    let b = &envelope.binding;
    if b.repo_id != candidate.repo_id()
        || b.task_id != candidate.task_id()
        || b.worktree_id != candidate.worktree_id()
        || b.requirement_ids != candidate.requirement_ids()
        || b.candidate_oid != candidate.candidate_oid()
        || b.base_oid != candidate.base_oid()
        || b.merge_group_id.as_deref() != candidate.merge_group_id()
        || b.source_snapshot_digest != format!("sha256:{}", candidate.source_snapshot_digest())
        || b.baseline_digest.as_deref() != candidate.baseline_digest()
    {
        return Err("candidate binding mismatch".into());
    }
    if !envelope
        .coverage
        .required_scopes
        .contains(&format!("git.scope:{}", candidate.binding_digest()))
    {
        return Err("missing full Git scope binding".into());
    }
    let expected_keys = [
        "advisory",
        "apiVersion",
        "authorization",
        "candidate",
        "changes",
        "mappingVersion",
    ];
    let object = bundle.domain.as_object().ok_or("invalid domain object")?;
    if object.len() != expected_keys.len()
        || object
            .keys()
            .any(|key| !expected_keys.contains(&key.as_str()))
    {
        return Err("unknown domain field".into());
    }
    let domain_bytes = serde_json::to_vec(&bundle.domain).map_err(|_| "invalid domain")?;
    if domain_bytes.len() > integration::MAX_ARTIFACT_BYTES || envelope.artifacts.domain.len() != 1
    {
        return Err("invalid domain coverage".into());
    }
    let reference = &envelope.artifacts.domain[0];
    if reference.digest != format!("sha256:{}", hash(&domain_bytes)) {
        return Err("domain digest mismatch".into());
    }
    if bundle.domain["apiVersion"] != "gitguard.domain/v1alpha1"
        || bundle.domain["mappingVersion"] != crate::evidence::projection::MAPPING_VERSION
        || bundle.domain["advisory"] != true
        || bundle.domain["authorization"] != "not-evaluated"
    {
        return Err("unsupported domain profile".into());
    }
    let domain_candidate: CandidateSnapshot =
        serde_json::from_value(bundle.domain["candidate"].clone())
            .map_err(|_| "invalid domain candidate")?;
    if domain_candidate.binding_digest() != candidate.binding_digest() {
        return Err("domain candidate mismatch".into());
    }
    let bytes = |value: &Option<serde_json::Value>| -> Result<Vec<u8>, String> {
        match value {
            Some(value) => serde_json::to_vec(value).map_err(|_| "invalid artifact".into()),
            None => Ok(vec![]),
        }
    };
    let contract = bytes(&bundle.contract)?;
    let facts = bytes(&bundle.facts)?;
    let report = bytes(&bundle.report)?;
    let result = evaluate_eligibility(
        envelope,
        ArtifactBytes {
            contract: &contract,
            facts: &facts,
            report: &report,
        },
        policy,
        provider,
        now,
        cause,
    );
    Ok(Consumption {
        result,
        candidate_digest: candidate.binding_digest(),
        policy_digest: policy.content_digest(),
        run_id: envelope.run_id.clone(),
    })
}
