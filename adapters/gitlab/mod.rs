//! Bounded GitLab REST v4 merge-train observation. No HTTP client, admission or grant.
use crate::{
    Repository,
    candidate::{CandidateRequest, CandidateSnapshot},
    scope::TaskScope,
    subject::SubjectRequest,
};
pub const PROFILE: &str = "gitguard.gitlab-rest-v4-train/v1";
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Unsupported,
    Schema,
    Binding,
    Unavailable,
    Mapping,
}
/// Controller-owned observation from its authenticated, current platform lookup.
/// Not deserializable: untrusted REST metadata must never construct this authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentTrain {
    pub profile: String,
    pub repo_id: String,
    pub project_id: u64,
    pub mr_id: u64,
    pub mr_iid: u64,
    pub train_id: u64,
    pub pipeline_id: u64,
    pub target_branch: String,
    pub pipeline_ref: String,
    pub candidate_oid: String,
    pub base_oid: String,
    pub members: Vec<String>,
    pub group_id: String,
    pub generation: u64,
    pub response_digest: String,
    pub merge_train_pipeline: bool,
}
pub trait CurrentLookup {
    fn current(&self, project_id: u64, mr_iid: u64) -> Result<Option<CurrentTrain>, Error>;
}
pub struct PreparedTrain {
    candidate: CandidateSnapshot,
    current: CurrentTrain,
}
impl PreparedTrain {
    pub fn candidate(&self) -> &CandidateSnapshot {
        &self.candidate
    }
    pub fn is_current(&self, port: &impl CurrentLookup) -> Result<bool, Error> {
        Ok(port
            .current(self.current.project_id, self.current.mr_iid)?
            .as_ref()
            == Some(&self.current))
    }
}
pub fn response_digest(raw: &[u8]) -> String {
    crate::subject::hash(raw)
}
pub fn prepare(
    raw: &[u8],
    repo: &Repository,
    expected: &CurrentTrain,
    scope: &TaskScope,
    worktree: &str,
    port: &impl CurrentLookup,
) -> Result<PreparedTrain, Error> {
    if raw.len() > 1_048_576 {
        return Err(Error::Schema);
    }
    if expected.profile != PROFILE
        || repo.object_format() != "sha1"
        || !expected.merge_train_pipeline
    {
        return Err(Error::Unsupported);
    }
    let label = |s: &str| !s.is_empty() && s.len() <= 512 && !s.chars().any(char::is_control);
    let oid = |s: &str| {
        s.len() == 40
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    };
    if repo.repo_id() != expected.repo_id
        || expected.generation == 0
        || expected.project_id == 0
        || expected.mr_id == 0
        || expected.mr_iid == 0
        || expected.train_id == 0
        || expected.pipeline_id == 0
        || !label(&expected.target_branch)
        || !label(&expected.group_id)
        || !label(worktree)
        || !label(&expected.pipeline_ref)
        || !oid(&expected.candidate_oid)
        || !oid(&expected.base_oid)
        || expected.members.is_empty()
        || expected.members.len() > 128
        || expected.members.iter().any(|m| !oid(m))
        || expected.candidate_oid == expected.base_oid
        || expected.members.contains(&expected.candidate_oid)
        || expected.response_digest != response_digest(raw)
    {
        return Err(Error::Binding);
    }
    // Current lookup is an independent controller port; the REST body cannot authenticate itself.
    if port.current(expected.project_id, expected.mr_iid)?.as_ref() != Some(expected) {
        return Err(Error::Unavailable);
    }
    let value = crate::api::unique_json(raw).map_err(|_| Error::Schema)?;
    let response: Train = serde_json::from_value(value).map_err(|_| Error::Schema)?;
    let pipeline = response.pipeline.ok_or(Error::Unsupported)?;
    if response.status != "fresh"
        || response.merge_request.state != "opened"
        || pipeline.status != "success"
        || pipeline.source != "merge_request_event"
    {
        return Err(Error::Unsupported);
    }
    if response.id != expected.train_id
        || response.target_branch != expected.target_branch
        || response.merge_request.id != expected.mr_id
        || response.merge_request.iid != expected.mr_iid
        || response.merge_request.project_id != expected.project_id
        || pipeline.id != expected.pipeline_id
        || pipeline.project_id != expected.project_id
        || pipeline.sha != expected.candidate_oid
        || pipeline.reference != expected.pipeline_ref
    {
        return Err(Error::Binding);
    }
    crate::evidence::freshness::admit(scope, 1_048_576).map_err(|_| Error::Mapping)?;
    let subject = repo
        .resolve_subject(SubjectRequest::Commit(expected.candidate_oid.clone()))
        .map_err(|_| Error::Mapping)?;
    let candidate = repo
        .prepare_candidate(
            &subject,
            scope,
            &CandidateRequest {
                worktree_id: worktree.into(),
                base_oid: expected.base_oid.clone(),
                merge_group_id: Some(expected.group_id.clone()),
                members: expected.members.clone(),
            },
        )
        .map_err(|_| Error::Mapping)?;
    // Recheck after local object resolution. A frozen observation is still advisory, never a grant.
    if port.current(expected.project_id, expected.mr_iid)?.as_ref() != Some(expected) {
        return Err(Error::Unavailable);
    }
    Ok(PreparedTrain {
        candidate,
        current: expected.clone(),
    })
}
#[derive(serde::Deserialize)]
struct Train {
    id: u64,
    target_branch: String,
    status: String,
    merge_request: MergeRequest,
    pipeline: Option<Pipeline>,
}
#[derive(serde::Deserialize)]
struct MergeRequest {
    id: u64,
    iid: u64,
    project_id: u64,
    state: String,
}
#[derive(serde::Deserialize)]
struct Pipeline {
    id: u64,
    project_id: u64,
    sha: String,
    #[serde(rename = "ref")]
    reference: String,
    source: String,
    status: String,
}
