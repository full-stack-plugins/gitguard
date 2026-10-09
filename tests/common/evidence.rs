//! Explicit fixture-only authority; never linked into production.
use super::*;
use gitguard::{
    Repository,
    candidate::{CandidateRequest, CandidateSnapshot},
    cli::CheckRequest,
    evidence::{
        envelope::{BoundCheck, CheckBundle},
        projection::FrozenPolicy,
    },
    scope::TaskScope,
};
use guardengine::{
    Enforcement,
    integration::{GuardRunEnvelope, eligibility::*},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::AtomicBool,
};
pub struct Evidence {
    pub source: Repo,
    pub repo: Repository,
    pub candidate: CandidateSnapshot,
    pub bundle: CheckBundle,
}
pub fn evidence(enforcement: Enforcement, dirty: bool, cancel: bool) -> Evidence {
    let source = Repo::new("sha1");
    let head = source.commit("a", "candidate");
    if dirty {
        std::fs::write(source.path().join("a"), "dirty").unwrap();
    }
    let policy = FrozenPolicy::new(enforcement);
    let bundle = BoundCheck::prepare(CheckRequest {
        api_version: "gitguard.check/v1alpha1".into(),
        repo_root: source.path().into(),
        repo_id: "repo".into(),
        candidate_oid: head,
        scope: TaskScope::advisory(
            "task",
            vec!["R".into()],
            vec![b"other".to_vec()],
            &policy.digest(),
            None,
        )
        .unwrap(),
        candidate: CandidateRequest {
            worktree_id: "w".into(),
            base_oid: source.base.clone(),
            merge_group_id: None,
            members: vec![],
        },
        contract: policy.contract(),
    })
    .unwrap()
    .run(&AtomicBool::new(cancel))
    .unwrap();
    let repo = Repository::discover(source.path(), "repo").unwrap();
    let candidate = serde_json::from_value(bundle.domain["candidate"].clone()).unwrap();
    Evidence {
        source,
        repo,
        candidate,
        bundle,
    }
}
pub fn policy(bundle: &CheckBundle) -> EligibilityPolicy {
    EligibilityPolicy {
        binding: bundle.envelope.binding.clone(),
        producer: bundle.envelope.producer.clone(),
        required_scopes: bundle.envelope.coverage.required_scopes.clone(),
        contract_digest: bundle
            .envelope
            .artifacts
            .contract
            .as_ref()
            .map(|r| r.digest.clone())
            .unwrap_or_else(|| format!("sha256:{}", "a".repeat(64))),
        action: "check-git-candidate".into(),
        producer_principals: BTreeSet::from(["fixture-ci".into()]),
        approval_principals: BTreeMap::from([(
            "scope-review".into(),
            BTreeSet::from(["fixture-reviewer".into()]),
        )]),
    }
}
pub struct FixtureAuthority {
    pub policy: EligibilityPolicy,
    pub principal: String,
    pub revoked: bool,
    pub expires: i64,
    pub available: bool,
}
impl FixtureAuthority {
    pub fn new(policy: EligibilityPolicy) -> Self {
        Self {
            policy,
            principal: "fixture-ci".into(),
            revoked: false,
            expires: i64::MAX,
            available: true,
        }
    }
}
impl AuthorityProvider for FixtureAuthority {
    fn verify_producer(
        &self,
        envelope: &GuardRunEnvelope,
        digest: &str,
    ) -> Result<ProducerRecord, AuthorityError> {
        if !self.available {
            return Err(AuthorityError::Unavailable);
        }
        Ok(ProducerRecord {
            principal: self.principal.clone(),
            producer: envelope.producer.clone(),
            envelope_digest: digest.into(),
            validity: Validity {
                issued_at: 0,
                expires_at: self.expires,
                revoked: self.revoked,
            },
        })
    }
    fn verify_approval(&self, reference: &str) -> Result<ApprovalRecord, AuthorityError> {
        if reference != "fixture:approval" {
            return Err(AuthorityError::Untrusted);
        }
        Ok(ApprovalRecord {
            principal: "fixture-reviewer".into(),
            purpose: "scope-review".into(),
            action: self.policy.action.clone(),
            binding: self.policy.binding.clone(),
            contract_digest: self.policy.contract_digest.clone(),
            validity: Validity {
                issued_at: 0,
                expires_at: self.expires,
                revoked: self.revoked,
            },
        })
    }
}
pub const NOW: i64 = 2_000_000_000;
