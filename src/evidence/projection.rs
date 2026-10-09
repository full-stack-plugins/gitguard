//! GitGuard-owned mapping; Git fields never extend the native engine wire objects.
use crate::{
    Repository,
    candidate::CandidateRequest,
    preflight::PreflightResult,
    subject::{SubjectRequest, hash},
};
use guardengine::{
    AnalyzerIdentity, Completeness, ContractMetadata, ContractSpec, Enforcement, GuardAssertion,
    GuardContract, GuardFact, GuardFacts, GuardRule, GuardSubject,
};
pub const MAPPING_VERSION: &str = "gitguard.mapping/v1alpha1";
pub const ANALYZER_ID: &str = "gitguard.git-scope";
pub const ANALYZER_VERSION: &str = "1";
#[derive(Clone)]
pub struct FrozenPolicy {
    contract: GuardContract,
}
impl FrozenPolicy {
    pub fn new(enforcement: Enforcement) -> Self {
        Self {
            contract: GuardContract {
                api_version: guardengine::API_VERSION.into(),
                kind: "GuardContract".into(),
                metadata: ContractMetadata {
                    id: "gitguard.scope".into(),
                    revision: "1".into(),
                },
                spec: ContractSpec {
                    rules: vec![GuardRule {
                        id: "scope.allowed-paths".into(),
                        description: "Changed paths must remain in frozen task scope".into(),
                        enforcement,
                        assertion: GuardAssertion::ForbidRelation {
                            subject: "git-candidate".into(),
                            predicate: "violates".into(),
                            object: "task-scope".into(),
                        },
                    }],
                },
            },
        }
    }
    pub fn from_contract(contract: GuardContract) -> Result<Self, String> {
        contract.validate().map_err(|_| "invalid engine contract")?;
        if contract.spec.rules.len() > 128
            || serde_json::to_vec(&contract)
                .map_err(|_| "contract serialization failed")?
                .len()
                > 65536
        {
            return Err("contract budget exceeded".into());
        }
        for rule in &contract.spec.rules {
            let GuardAssertion::ForbidRelation {
                subject,
                predicate,
                object,
            } = &rule.assertion;
            if subject != "git-candidate" || predicate != "violates" || object != "task-scope" {
                return Err("unsupported Git mapping relation".into());
            }
        }
        Ok(Self { contract })
    }
    pub fn digest(&self) -> String {
        guardengine::digest_json(&self.contract)
            .expect("closed contract serialization")
            .trim_start_matches("sha256:")
            .into()
    }
    pub fn contract(&self) -> GuardContract {
        self.contract.clone()
    }
}
pub fn project(
    repo: &Repository,
    result: &PreflightResult,
    policy: &FrozenPolicy,
) -> Result<GuardFacts, String> {
    let candidate = &result.candidate;
    candidate.validate(repo).map_err(|_| "invalid candidate")?;
    if candidate.policy_digest() != policy.digest() {
        return Err("frozen policy digest mismatch".into());
    }
    let request = CandidateRequest {
        worktree_id: candidate.worktree_id().into(),
        base_oid: candidate.base_oid().into(),
        merge_group_id: candidate.merge_group_id().map(str::to_owned),
        members: candidate.members().to_vec(),
    };
    let subject = repo
        .resolve_subject(SubjectRequest::Commit(candidate.candidate_oid().into()))
        .map_err(|_| "source observation failed")?;
    let expected = repo
        .prepare_candidate(&subject, &result.scope, &request)
        .map_err(|_| "scope binding invalid")?;
    if expected.binding_digest() != candidate.binding_digest() {
        return Err("scope binding mismatch".into());
    }
    // Reobserve Git-owned facts instead of trusting mutable caller-populated ChangeSet/complete.
    let changes = repo
        .scan_scope(
            candidate.base_oid(),
            candidate.candidate_oid(),
            &result.scope,
        )
        .map_err(|_| "Git scope scan failed")?;
    let mut diagnostics = vec![];
    if !candidate.clean() {
        diagnostics.push("source.not_clean".into())
    }
    if !changes.missing_coverage.is_empty() {
        diagnostics.push("submodule.contents_missing".into())
    }
    let facts = GuardFacts {
        api_version: guardengine::API_VERSION.into(),
        kind: "GuardFacts".into(),
        analyzer: AnalyzerIdentity {
            id: ANALYZER_ID.into(),
            version: ANALYZER_VERSION.into(),
        },
        subject: GuardSubject {
            id: format!("git-scope:{}", candidate.binding_digest()),
            snapshot_digest: format!("sha256:{}", candidate.source_snapshot_digest()),
        },
        completeness: if diagnostics.is_empty() {
            Completeness::Complete
        } else {
            Completeness::Partial
        },
        facts: changes
            .violations
            .iter()
            .map(|path| GuardFact {
                subject: "git-candidate".into(),
                predicate: "violates".into(),
                object: "task-scope".into(),
                source: format!("git-path-sha256:{}", hash(path)),
            })
            .collect(),
        diagnostics,
    };
    facts.validate().map_err(|_| "invalid mapped facts")?;
    let fact_bytes = serde_json::to_vec(&facts)
        .map_err(|_| "fact serialization failed")?
        .len();
    let rules = policy.contract.spec.rules.len();
    if fact_bytes > 1_048_576
        || facts.facts.len() > 4096
        || rules.saturating_mul(facts.facts.len()) > 100_000
        || rules.saturating_mul(
            fact_bytes
                .saturating_add(
                    facts
                        .facts
                        .len()
                        .saturating_mul(std::mem::size_of::<GuardFact>()),
                )
                .saturating_add(1024),
        ) > 8_388_608
    {
        return Err("evaluation budget exceeded".into());
    }
    Ok(facts)
}
