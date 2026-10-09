//! Controller-frozen local source policy; never approval authority.
use super::{TaskScope, digest_valid, path_valid};
use crate::{Diagnostic, Repository, Result};
use serde::{Deserialize, Serialize};
use std::{os::unix::ffi::OsStrExt, path::PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct ImmutableScopeSource {
    pub commit_oid: String,
    pub path: Vec<u8>,
    /// SHA-256 of exact committed bytes, independently pinned by the controller.
    pub digest: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct ProtectedScopeRequest {
    pub repo_id: String,
    pub task_id: String,
    pub requirement_ids: Vec<String>,
    pub allowed_paths: Vec<Vec<u8>>,
    pub policy: ImmutableScopeSource,
    pub baseline: Option<ImmutableScopeSource>,
}
#[derive(Debug, Serialize)]
struct SourceRecord {
    reference: ImmutableScopeSource,
    blob_oid: String,
    mode: String,
}
#[derive(Debug, Serialize)]
struct Descriptor {
    schema_version: &'static str,
    trust_profile: &'static str,
    advisory: bool,
    repo_id: String,
    root: Vec<u8>,
    common_dir: Vec<u8>,
    object_format: String,
    task_id: String,
    requirement_ids: Vec<String>,
    allowed_paths: Vec<Vec<u8>>,
    policy: SourceRecord,
    baseline: Option<SourceRecord>,
}
/// A controller-owned value constructed only from verified immutable sources.
/// It is not deserializable and does not authenticate its caller.
/// ```compile_fail
/// let forged: gitguard::scope::ProtectedTaskScope = serde_json::from_str("{}").unwrap();
/// ```
#[derive(Debug)]
pub struct ProtectedTaskScope {
    scope: TaskScope,
    root: PathBuf,
    common_dir: PathBuf,
    descriptor: Descriptor,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    schema_version: String,
    task_id: String,
    requirement_ids: Vec<String>,
    allowed_paths: Vec<Vec<u8>>,
}
fn metadata(request: &ProtectedScopeRequest, repo: &Repository) -> Result<()> {
    let mut total = 0usize;
    let mut add = |bytes: &[u8], limit: usize| -> Result<()> {
        if bytes.len() > limit {
            return Err(Diagnostic::LimitExceeded);
        }
        total = total
            .checked_add(bytes.len())
            .ok_or(Diagnostic::LimitExceeded)?;
        if total > 65536 {
            return Err(Diagnostic::LimitExceeded);
        }
        Ok(())
    };
    add(request.repo_id.as_bytes(), 128)?;
    add(request.task_id.as_bytes(), 256)?;
    add(repo.root().as_os_str().as_bytes(), 4096)?;
    add(repo.common_dir().as_os_str().as_bytes(), 4096)?;
    if request.requirement_ids.len() > 256 || request.allowed_paths.len() > 256 {
        return Err(Diagnostic::LimitExceeded);
    }
    for id in &request.requirement_ids {
        add(id.as_bytes(), 256)?;
    }
    for p in &request.allowed_paths {
        add(p, 4096)?;
    }
    for s in std::iter::once(&request.policy).chain(request.baseline.iter()) {
        add(s.commit_oid.as_bytes(), 64)?;
        add(&s.path, 4096)?;
        add(s.digest.as_bytes(), 64)?;
        if !path_valid(&s.path) || !digest_valid(&s.digest) {
            return Err(Diagnostic::InvalidScope);
        }
    }
    if request.repo_id != repo.repo_id()
        || request.task_id.is_empty()
        || request.requirement_ids.is_empty()
        || request.requirement_ids.iter().any(|x| x.is_empty())
        || request.requirement_ids.windows(2).any(|w| w[0] >= w[1])
        || request.allowed_paths.iter().any(|p| !path_valid(p))
        || request.allowed_paths.windows(2).any(|w| w[0] >= w[1])
    {
        return Err(Diagnostic::InvalidScope);
    }
    Ok(())
}
fn source(repo: &Repository, expected: &ImmutableScopeSource) -> Result<(SourceRecord, Vec<u8>)> {
    let files = repo.read_commit_files(&expected.commit_oid)?;
    let file = files
        .iter()
        .find(|f| f.path() == expected.path)
        .ok_or(Diagnostic::InvalidScope)?;
    if file.contents().len() > 65536 {
        return Err(Diagnostic::LimitExceeded);
    }
    if crate::subject::hash(file.contents()) != expected.digest {
        return Err(Diagnostic::InvalidScope);
    }
    Ok((
        SourceRecord {
            reference: expected.clone(),
            blob_oid: file.object_oid().into(),
            mode: file.mode().into(),
        },
        file.contents().to_vec(),
    ))
}
impl ProtectedTaskScope {
    pub fn freeze(repo: &Repository, request: &ProtectedScopeRequest) -> Result<Self> {
        metadata(request, repo)?;
        let (policy, bytes) = source(repo, &request.policy)?;
        let parsed: Policy =
            serde_json::from_slice(&bytes).map_err(|_| Diagnostic::InvalidScope)?;
        if parsed.schema_version != "gitguard.scope-policy/v1alpha1"
            || parsed.task_id != request.task_id
            || parsed.requirement_ids != request.requirement_ids
            || parsed.allowed_paths != request.allowed_paths
        {
            return Err(Diagnostic::InvalidScope);
        }
        let baseline = request
            .baseline
            .as_ref()
            .map(|s| source(repo, s).map(|x| x.0))
            .transpose()?;
        let descriptor = Descriptor {
            schema_version: "gitguard.protected-scope/v1alpha1",
            trust_profile: "local-controller-source-only",
            advisory: true,
            repo_id: request.repo_id.clone(),
            root: repo.root().as_os_str().as_bytes().to_vec(),
            common_dir: repo.common_dir().as_os_str().as_bytes().to_vec(),
            object_format: repo.object_format().into(),
            task_id: request.task_id.clone(),
            requirement_ids: request.requirement_ids.clone(),
            allowed_paths: request.allowed_paths.clone(),
            policy,
            baseline,
        };
        let digest = crate::subject::hash(
            &serde_json::to_vec(&descriptor).map_err(|_| Diagnostic::InvalidScope)?,
        );
        let scope = TaskScope::advisory(
            &request.task_id,
            request.requirement_ids.clone(),
            request.allowed_paths.clone(),
            &digest,
            None,
        )?;
        Ok(Self {
            scope,
            root: repo.root().into(),
            common_dir: repo.common_dir().into(),
            descriptor,
        })
    }
    pub fn digest(&self) -> &str {
        &self.scope.policy_digest
    }
    pub fn task_scope(&self) -> &TaskScope {
        &self.scope
    }
    /// Audit descriptor only. Reading or recomputing this JSON does not recover authority.
    pub fn descriptor_json(&self) -> Result<Vec<u8>> {
        serde_json::to_vec(&self.descriptor).map_err(|_| Diagnostic::InvalidScope)
    }
    pub fn validate_sources(&self, repo: &Repository) -> Result<()> {
        if self.root != repo.root()
            || self.common_dir != repo.common_dir()
            || self.descriptor.repo_id != repo.repo_id()
            || self.descriptor.object_format != repo.object_format()
        {
            return Err(Diagnostic::InvalidBinding);
        }
        for old in std::iter::once(&self.descriptor.policy).chain(self.descriptor.baseline.iter()) {
            let (current, _) = source(repo, &old.reference)?;
            if current.blob_oid != old.blob_oid || current.mode != old.mode {
                return Err(Diagnostic::InvalidBinding);
            }
        }
        Ok(())
    }
}
