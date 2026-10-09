//! Bounded process-local audit port, separate from immutable technical history.
use super::{
    consume::Consumption,
    freshness::{FreshnessAudit, FreshnessObservation},
};
use guardengine::integration::eligibility::AuditRecord;
use std::{collections::BTreeSet, sync::Mutex};
#[derive(Clone)]
pub struct AuditPolicy {
    pub readers: BTreeSet<String>,
    pub deleters: BTreeSet<String>,
    pub retention_seconds: u64,
    pub max_records: usize,
}
/// Implementations must authenticate the caller externally, not read a request Boolean.
pub trait AuditIdentityProvider {
    fn authenticated_principal(&self) -> Result<String, String>;
}
pub struct UnavailableAuditIdentity;
impl AuditIdentityProvider for UnavailableAuditIdentity {
    fn authenticated_principal(&self) -> Result<String, String> {
        Err("audit identity unavailable".into())
    }
}
pub struct AuditLog {
    policy: AuditPolicy,
    records: Mutex<AuditState>,
}
#[derive(Default)]
struct AuditState {
    technical: Vec<AuditRecord>,
    freshness: Vec<FreshnessAudit>,
    last_time: Option<i64>,
}
impl AuditState {
    fn clock(&mut self, now: i64) -> Result<(), String> {
        if now < 0 || self.last_time.is_some_and(|last| now < last) {
            return Err("audit clock rollback".into());
        }
        self.last_time = Some(now);
        Ok(())
    }
    fn len(&self) -> usize {
        self.technical.len() + self.freshness.len()
    }
}
impl AuditLog {
    pub fn new(policy: AuditPolicy) -> Result<Self, String> {
        super::freshness::admit(&(&policy.readers, &policy.deleters), 64 * 1024)?;
        if policy.readers.is_empty()
            || policy
                .readers
                .iter()
                .chain(policy.deleters.iter())
                .any(|s| s.trim().is_empty())
            || policy.retention_seconds == 0
            || policy.retention_seconds > i64::MAX as u64
            || policy.max_records == 0
            || policy.max_records > 10_000
        {
            return Err("invalid audit policy".into());
        }
        Ok(Self {
            policy,
            records: Mutex::new(AuditState::default()),
        })
    }
    pub fn append(&self, consumption: &Consumption) -> Result<(), String> {
        let mut records = self.records.lock().map_err(|_| "audit unavailable")?;
        if records.len() >= self.policy.max_records {
            return Err("audit capacity exceeded".into());
        }
        super::freshness::admit(&consumption.result.audit, 4096)?;
        records.clock(consumption.result.audit.timestamp)?;
        records.technical.push(consumption.result.audit.clone());
        Ok(())
    }
    pub fn read(&self, identity: &dyn AuditIdentityProvider) -> Result<Vec<AuditRecord>, String> {
        let principal = identity
            .authenticated_principal()
            .map_err(|_| "audit identity unavailable")?;
        if !self.policy.readers.contains(&principal) {
            return Err("audit access denied".into());
        }
        Ok(self
            .records
            .lock()
            .map_err(|_| "audit unavailable")?
            .technical
            .clone())
    }
    /// Append only a private observation from FreshnessSession, never caller-made audit JSON.
    pub fn append_freshness(&self, observation: &FreshnessObservation) -> Result<(), String> {
        super::freshness::admit(observation.audit(), 4096)?;
        let mut records = self.records.lock().map_err(|_| "audit unavailable")?;
        if records.len() >= self.policy.max_records {
            return Err("audit capacity exceeded".into());
        }
        records.clock(observation.audit().timestamp)?;
        records.freshness.push(observation.audit().clone());
        Ok(())
    }
    pub fn read_freshness(
        &self,
        identity: &dyn AuditIdentityProvider,
    ) -> Result<Vec<FreshnessAudit>, String> {
        let principal = identity
            .authenticated_principal()
            .map_err(|_| "audit identity unavailable")?;
        if !self.policy.readers.contains(&principal) {
            return Err("audit access denied".into());
        }
        Ok(self
            .records
            .lock()
            .map_err(|_| "audit unavailable")?
            .freshness
            .clone())
    }
    /// Deletes only this log's expired sanitized audit rows; never technical reports/history.
    pub fn purge(&self, identity: &dyn AuditIdentityProvider, now: i64) -> Result<usize, String> {
        let principal = identity
            .authenticated_principal()
            .map_err(|_| "audit identity unavailable")?;
        if !self.policy.deleters.contains(&principal) {
            return Err("retention access denied".into());
        }
        let mut records = self.records.lock().map_err(|_| "audit unavailable")?;
        records.clock(now)?;
        let old = records.len();
        records.technical.retain(|r| {
            now < r.timestamp
                || now.saturating_sub(r.timestamp) < self.policy.retention_seconds as i64
        });
        records
            .freshness
            .retain(|r| now.saturating_sub(r.timestamp) < self.policy.retention_seconds as i64);
        Ok(old - records.len())
    }
}
