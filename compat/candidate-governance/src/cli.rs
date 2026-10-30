use crate::{candidate::CandidateRequest, scope::TaskScope};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckRequest {
    pub api_version: String,
    pub repo_root: PathBuf,
    pub repo_id: String,
    pub candidate_oid: String,
    pub scope: TaskScope,
    pub candidate: CandidateRequest,
    pub contract: guardengine::GuardContract,
}
