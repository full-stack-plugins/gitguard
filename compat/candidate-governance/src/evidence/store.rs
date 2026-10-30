//! Atomic process-local history only; deliberately rejects durable storage requirements.
use super::{consume::Consumption, freshness::ReuseKey};
use guardengine::integration::attempt_store::{
    AttemptRecord, AttemptStore, InMemoryAttemptStore, Target,
};
use std::sync::{Arc, Mutex};
#[derive(Clone, Copy)]
pub enum StorageRequirement {
    ProcessLocal,
    Durable,
}
pub struct Ticket {
    key: ReuseKey,
    run_id: String,
    generation: u64,
    owner: Arc<Mutex<InMemoryAttemptStore>>,
}
pub struct LocalHistory {
    state: Arc<Mutex<InMemoryAttemptStore>>,
}
impl LocalHistory {
    pub fn new(requirement: StorageRequirement) -> Result<Self, String> {
        if matches!(requirement, StorageRequirement::Durable) {
            return Err("durable storage unsupported".into());
        }
        Ok(Self {
            state: Arc::new(Mutex::new(InMemoryAttemptStore::default())),
        })
    }
    pub fn begin(
        &self,
        key: &ReuseKey,
        expected_generation: u64,
        run_id: &str,
    ) -> Result<Ticket, String> {
        let generation = self
            .state
            .lock()
            .map_err(|_| "store unavailable")?
            .advance(
                key.target.clone(),
                expected_generation,
                key.digest().into(),
                run_id.into(),
            )
            .map_err(|_| "attempt registration rejected")?;
        Ok(Ticket {
            key: key.clone(),
            run_id: run_id.into(),
            generation,
            owner: self.state.clone(),
        })
    }
    pub fn complete(&self, ticket: &Ticket, consumption: &Consumption) -> Result<(), String> {
        if !Arc::ptr_eq(&self.state, &ticket.owner)
            || ticket.run_id != consumption.run_id
            || ticket.key.candidate_digest != consumption.candidate_digest
            || ticket.key.policy_digest != consumption.policy_digest
            || consumption.reuse_digest.as_deref() != Some(ticket.key.digest())
        {
            return Err("consumption does not match attempt".into());
        }
        self.state
            .lock()
            .map_err(|_| "store unavailable")?
            .append(AttemptRecord {
                run_id: ticket.run_id.clone(),
                target: ticket.key.target.clone(),
                generation: ticket.generation,
                content_digest: ticket.key.digest().into(),
                envelope_digest: consumption.result.audit.envelope_digest.clone(),
                eligible: consumption.result.eligible,
            })
            .map_err(|_| "history append rejected")?;
        Ok(())
    }
    pub fn publish(&self, ticket: &Ticket) -> Result<(), String> {
        if !Arc::ptr_eq(&self.state, &ticket.owner) {
            return Err("ticket belongs to another store".into());
        }
        self.state
            .lock()
            .map_err(|_| "store unavailable")?
            .publish(&ticket.run_id)
            .map_err(|_| "stale or incomplete attempt".into())
    }
    /// Archival observation, not cached authority. Consumers must freshly call consume.
    pub fn current(&self, target: &Target) -> Result<Option<AttemptRecord>, String> {
        Ok(self
            .state
            .lock()
            .map_err(|_| "store unavailable")?
            .current(target)
            .cloned())
    }
    pub fn history(&self, target: &Target) -> Result<Vec<AttemptRecord>, String> {
        Ok(self
            .state
            .lock()
            .map_err(|_| "store unavailable")?
            .history(target)
            .into_iter()
            .cloned()
            .collect())
    }
}
