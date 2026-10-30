//! Historical local controller acknowledgement or explicit reftable platform receipt.
//! No current-ref inference, remote authentication, retry or compensation is performed.
use super::{
    grant::GrantSession,
    intent::{IntentState, IntentStore},
    platform_write::ProtectedBareTarget,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryState {
    /// No journal record exists. This does not prove that the platform was never written.
    NotRecorded,
    /// This controller durably prepared, but has never claimed this operation.
    NotAttempted,
    /// A durable acknowledgement of the actual successful write, not current target state.
    Applied {
        operation_id: String,
        request_digest: String,
        candidate_oid: String,
    },
    RecoveryRequired,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryError {
    Target,
    Intent,
}

/// Query is read-only, and needs no still-live grant to inspect historical completion.
/// Dispatch/retry remains exclusively through apply, which revalidates a fresh grant.
pub fn reconcile(
    session: &GrantSession,
    intents: &IntentStore,
    target: &ProtectedBareTarget,
) -> Result<RecoveryState, RecoveryError> {
    target.unchanged().map_err(|_| RecoveryError::Target)?;
    let historical = reconcile_identity(session, intents, target.identity())?;
    if historical == RecoveryState::RecoveryRequired {
        let expected = session.expectation();
        let receipt = intents
            .lookup_for_platform(expected, target.identity())
            .map_err(|_| RecoveryError::Intent)?
            .ok_or(RecoveryError::Intent)?;
        if target
            .confirms_receipt(expected, receipt.request_digest())
            .map_err(|_| RecoveryError::Target)?
        {
            return Ok(RecoveryState::Applied {
                operation_id: receipt.operation_id().into(),
                request_digest: receipt.request_digest().into(),
                candidate_oid: expected.candidate_oid().into(),
            });
        }
    }
    Ok(historical)
}

pub(crate) fn reconcile_identity(
    session: &GrantSession,
    intents: &IntentStore,
    identity: &str,
) -> Result<RecoveryState, RecoveryError> {
    let expected = session.expectation();
    let receipt = intents
        .lookup_for_platform(expected, identity)
        .map_err(|_| RecoveryError::Intent)?;
    Ok(match receipt {
        None => RecoveryState::NotRecorded,
        Some(receipt) => match receipt.state() {
            IntentState::Prepared => RecoveryState::NotAttempted,
            IntentState::AttemptRecorded | IntentState::RecoveryRequired => {
                RecoveryState::RecoveryRequired
            }
            IntentState::Applied => RecoveryState::Applied {
                operation_id: receipt.operation_id().into(),
                request_digest: receipt.request_digest().into(),
                candidate_oid: expected.candidate_oid().into(),
            },
        },
    })
}
