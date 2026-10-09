//! Read-only controller grant observations. No executor conversion or writer is linked.
use crate::{Repository, candidate::CandidateSnapshot};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, sync::Mutex};
const VERSION: &str = "gitguard.operation-grant/v1alpha1";
const RECORD_BYTES: usize = 16 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrantAction {
    CreateWorktree,
    CreateBranch,
    Push,
    Merge,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationGrant {
    pub api_version: String,
    pub issuer: String,
    pub actor: String,
    pub action: GrantAction,
    pub repo_id: String,
    pub candidate_oid: String,
    pub binding_digest: String,
    pub target: String,
    #[serde(deserialize_with = "required_nullable")]
    pub expected_target_oid: Option<String>,
    pub operation_id: String,
    pub not_before: i64,
    pub expires_at: i64,
    pub revoked: bool,
}
fn required_nullable<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(d)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrantError {
    Budget,
    InvalidRecord,
    InvalidExpectation,
    WrongAdapter,
    AuthorityUnavailable,
    ClockUnavailable,
    ClockRollback,
    Superseded,
    Mismatch,
    Inactive,
    Unavailable,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortError {
    Untrusted,
    Unavailable,
}
/// Protected controller selects/authenticates this adapter. An ID is not authentication.
pub trait GrantAuthority {
    fn adapter_id(&self) -> &str;
    fn authenticate(&self, reference: &str) -> Result<OperationGrant, PortError>;
}
/// Protected controller UTC Unix seconds; candidate/system-clock claims are not accepted.
pub trait GrantClock {
    fn adapter_id(&self) -> &str;
    fn now(&self) -> Result<i64, PortError>;
}
#[derive(Clone, Serialize)]
pub struct GrantPolicy {
    pub authority_adapter: String,
    pub clock_adapter: String,
    pub allowed_issuers: BTreeSet<String>,
    pub max_lifetime_seconds: i64,
}
#[derive(Clone, Serialize)]
pub struct GrantRequest {
    pub actor: String,
    pub action: GrantAction,
    pub target: String,
    pub expected_target_oid: Option<String>,
    pub operation_id: String,
}
fn text(s: &str, max: usize) -> bool {
    !s.trim().is_empty() && s.len() <= max && !s.chars().any(char::is_control)
}
fn oid(s: &str) -> bool {
    [40, 64].contains(&s.len())
        && s.bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn portable(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 256
        && !s.contains("..")
        && s.split('/').all(|part| {
            !part.is_empty()
                && !part.starts_with('.')
                && !part.ends_with('.')
                && !part.ends_with(".lock")
                && part
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
        })
}
fn target(action: GrantAction, name: &str, expected: Option<&str>) -> bool {
    let path = match action {
        GrantAction::CreateWorktree => name,
        _ => match name.strip_prefix("refs/heads/") {
            Some(s) => s,
            None => return false,
        },
    };
    if name.len() > 256 || !portable(path) {
        return false;
    }
    match action {
        GrantAction::CreateWorktree | GrantAction::CreateBranch => expected.is_none(),
        GrantAction::Push | GrantAction::Merge => expected.is_some_and(oid),
    }
}
fn admit<T: Serialize>(v: &T, limit: usize) -> Result<(), GrantError> {
    crate::evidence::freshness::admit(v, limit).map_err(|_| GrantError::Budget)
}
impl OperationGrant {
    /// Strict wire parsing only: a decoded grant cannot authenticate itself.
    pub fn from_json(raw: &[u8]) -> Result<Self, GrantError> {
        if raw.len() > RECORD_BYTES {
            return Err(GrantError::Budget);
        }
        let record: Self = serde_json::from_slice(raw).map_err(|_| GrantError::InvalidRecord)?;
        record.validate_shape()?;
        Ok(record)
    }
    fn validate_shape(&self) -> Result<(), GrantError> {
        admit(self, RECORD_BYTES)?;
        if self.api_version != VERSION
            || !text(&self.issuer, 128)
            || !text(&self.actor, 128)
            || !text(&self.repo_id, 256)
            || !text(&self.operation_id, 128)
            || !oid(&self.candidate_oid)
            || !crate::scope::digest_valid(&self.binding_digest)
            || !target(
                self.action,
                &self.target,
                self.expected_target_oid.as_deref(),
            )
            || self
                .expected_target_oid
                .as_ref()
                .is_some_and(|oid| oid.len() != self.candidate_oid.len())
            || self.not_before < 0
            || self.expires_at <= self.not_before
        {
            return Err(GrantError::InvalidRecord);
        }
        Ok(())
    }
}
/// Frozen independently by the protected controller before authority is queried.
pub struct GrantExpectation {
    request: GrantRequest,
    policy: GrantPolicy,
    repo_id: String,
    candidate_oid: String,
    binding_digest: String,
    digest: String,
}
impl GrantExpectation {
    pub fn freeze(
        repo: &Repository,
        candidate: &CandidateSnapshot,
        request: GrantRequest,
        policy: GrantPolicy,
    ) -> Result<Self, GrantError> {
        admit(&(candidate, &request, &policy), 1024 * 1024)?;
        if !text(&request.actor, 128)
            || !text(&request.operation_id, 128)
            || !target(
                request.action,
                &request.target,
                request.expected_target_oid.as_deref(),
            )
            || request
                .expected_target_oid
                .as_ref()
                .is_some_and(|oid| oid.len() != candidate.candidate_oid().len())
            || !text(&policy.authority_adapter, 128)
            || !text(&policy.clock_adapter, 128)
            || policy.allowed_issuers.is_empty()
            || policy.allowed_issuers.len() > 128
            || policy.allowed_issuers.iter().any(|s| !text(s, 128))
            || !(1..=86400).contains(&policy.max_lifetime_seconds)
            || !text(candidate.repo_id(), 256)
        {
            return Err(GrantError::InvalidExpectation);
        }
        candidate
            .validate(repo)
            .map_err(|_| GrantError::InvalidExpectation)?;
        let binding_digest = candidate.binding_digest();
        let digest = guardengine::digest_json(&(
            VERSION,
            &request,
            &policy,
            candidate.repo_id(),
            candidate.candidate_oid(),
            &binding_digest,
        ))
        .map_err(|_| GrantError::InvalidExpectation)?;
        Ok(Self {
            request,
            policy,
            repo_id: candidate.repo_id().into(),
            candidate_oid: candidate.candidate_oid().into(),
            binding_digest,
            digest,
        })
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
    fn check(&self, grant: &OperationGrant, now: i64) -> Result<(), GrantError> {
        grant.validate_shape()?;
        if !self.policy.allowed_issuers.contains(&grant.issuer)
            || grant.actor != self.request.actor
            || grant.action != self.request.action
            || grant.repo_id != self.repo_id
            || grant.candidate_oid != self.candidate_oid
            || grant.binding_digest != self.binding_digest
            || grant.target != self.request.target
            || grant.expected_target_oid != self.request.expected_target_oid
            || grant.operation_id != self.request.operation_id
        {
            return Err(GrantError::Mismatch);
        }
        if grant.revoked
            || now < grant.not_before
            || now >= grant.expires_at
            || grant.expires_at.saturating_sub(grant.not_before) > self.policy.max_lifetime_seconds
        {
            return Err(GrantError::Inactive);
        }
        Ok(())
    }
}
#[derive(Default)]
struct Watermark {
    time: Option<i64>,
    generation: u64,
}
impl Watermark {
    fn advance(&mut self, now: i64) -> Result<(), GrantError> {
        if now < 0 || self.time.is_some_and(|t| now < t) {
            return Err(GrantError::ClockRollback);
        }
        self.time = Some(now);
        Ok(())
    }
}
pub struct GrantSession {
    expected: GrantExpectation,
    watermark: Mutex<Watermark>,
}
/// A past read-only observation, not a writer capability. Apply must revalidate and CAS.
pub struct GrantObservation {
    expectation_digest: String,
    reference_digest: String,
    observed_at: i64,
    expires_at: i64,
}
impl GrantObservation {
    pub fn expectation_digest(&self) -> &str {
        &self.expectation_digest
    }
    pub fn reference_digest(&self) -> &str {
        &self.reference_digest
    }
    pub fn observed_at(&self) -> i64 {
        self.observed_at
    }
    pub fn expires_at(&self) -> i64 {
        self.expires_at
    }
}
impl GrantSession {
    pub fn new(expected: GrantExpectation) -> Self {
        Self {
            expected,
            watermark: Mutex::new(Watermark::default()),
        }
    }
    pub fn validate(
        &self,
        reference: &str,
        authority: &dyn GrantAuthority,
        clock: &dyn GrantClock,
    ) -> Result<GrantObservation, GrantError> {
        if !text(reference, 2048) {
            return Err(GrantError::InvalidExpectation);
        }
        if authority.adapter_id() != self.expected.policy.authority_adapter
            || clock.adapter_id() != self.expected.policy.clock_adapter
        {
            return Err(GrantError::WrongAdapter);
        }
        let start = clock.now().map_err(|_| GrantError::ClockUnavailable)?;
        let generation = {
            let mut w = self.watermark.lock().map_err(|_| GrantError::Unavailable)?;
            w.advance(start)?;
            w.generation = w.generation.checked_add(1).ok_or(GrantError::Unavailable)?;
            w.generation
        };
        // No session lock across external ports: callbacks may safely reenter.
        let record = authority.authenticate(reference);
        let finish = clock.now().map_err(|_| GrantError::ClockUnavailable)?;
        let mut w = self.watermark.lock().map_err(|_| GrantError::Unavailable)?;
        w.advance(finish)?;
        if w.generation != generation {
            return Err(GrantError::Superseded);
        }
        let record = record.map_err(|_| GrantError::AuthorityUnavailable)?;
        self.expected.check(&record, finish)?;
        Ok(GrantObservation {
            expectation_digest: self.expected.digest.clone(),
            reference_digest: crate::subject::hash(reference.as_bytes()),
            observed_at: finish,
            expires_at: record.expires_at,
        })
    }
}
