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

/// Local controller profile. All candidate wire fields remain advisory v1alpha2.
pub fn preflight_protected(
    repo: &Repository,
    subject: SubjectRequest,
    scope: &crate::scope::ProtectedTaskScope,
    request: &CandidateRequest,
) -> Result<PreflightResult> {
    // Borrowed admission before source reads or legacy candidate cloning/hashing.
    if request.worktree_id.len() > 256
        || request.base_oid.len() > 64
        || request
            .merge_group_id
            .as_ref()
            .is_some_and(|s| s.len() > 256)
        || request.members.len() > 64
        || request.members.iter().any(|s| s.len() > 64)
        || matches!(&subject, SubjectRequest::Commit(s) | SubjectRequest::TreePreview(s) if s.len() > 64)
    {
        return Err(crate::Diagnostic::LimitExceeded);
    }
    scope.validate_sources(repo)?;
    preflight(repo, subject, scope.task_scope(), request)
}
