use crate::{
    Repository, Result,
    candidate::{CandidateRequest, CandidateSnapshot},
    git::diff::ChangeSet,
    scope::TaskScope,
    subject::SubjectRequest,
};
pub struct PreflightResult {
    pub candidate: CandidateSnapshot,
    pub changes: ChangeSet,
    pub scope: TaskScope,
    pub complete: bool,
}
pub fn preflight(
    repo: &Repository,
    subject: SubjectRequest,
    scope: &TaskScope,
    request: &CandidateRequest,
) -> Result<PreflightResult> {
    let subject = repo.resolve_subject(subject)?;
    let candidate = repo.prepare_candidate(&subject, scope, request)?;
    let changes = repo.scan_scope(&request.base_oid, candidate.candidate_oid(), scope)?;
    let complete =
        candidate.clean() && changes.violations.is_empty() && changes.missing_coverage.is_empty();
    Ok(PreflightResult {
        candidate,
        changes,
        scope: scope.clone(),
        complete,
    })
}
