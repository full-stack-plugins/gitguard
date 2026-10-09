//! Bounded process-local audit port, separate from immutable technical history.
use super::consume::Consumption;
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
    records: Mutex<Vec<AuditRecord>>,
}
impl AuditLog {
    pub fn new(policy: AuditPolicy) -> Result<Self, String> {
        if policy.readers.is_empty()
            || policy
                .readers
                .iter()
                .chain(policy.deleters.iter())
                .any(|s| s.trim().is_empty())
            || policy.retention_seconds == 0
            || policy.retention_seconds > i64::MAX as u64
            || policy.max_records == 0
        {
            return Err("invalid audit policy".into());
        }
        Ok(Self {
            policy,
            records: Mutex::new(vec![]),
        })
    }
    pub fn append(&self, consumption: &Consumption) -> Result<(), String> {
        let mut records = self.records.lock().map_err(|_| "audit unavailable")?;
        if records.len() >= self.policy.max_records {
            return Err("audit capacity exceeded".into());
        }
        records.push(consumption.result.audit.clone());
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
        let old = records.len();
        records.retain(|r| {
            now < r.timestamp
                || now.saturating_sub(r.timestamp) < self.policy.retention_seconds as i64
        });
        Ok(old - records.len())
    }
}
