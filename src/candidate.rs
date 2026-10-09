use crate::{
    Diagnostic, Repository, Result,
    scope::{TaskScope, digest_valid, path_valid},
    subject::{FrozenSubject, SubjectRequest, hash},
};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateRequest {
    pub worktree_id: String,
    pub base_oid: String,
    pub merge_group_id: Option<String>,
    pub members: Vec<String>,
}
/// Local observation only: this record does not authenticate its caller or grant admission.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateSnapshot {
    schema_version: String,
    repo_id: String,
    task_id: String,
    worktree_id: String,
    requirement_ids: Vec<String>,
    allowed_paths: Vec<Vec<u8>>,
    object_format: String,
    candidate_oid: String,
    base_oid: String,
    #[serde(deserialize_with = "required_nullable")]
    merge_group_id: Option<String>,
    members: Vec<String>,
    source_snapshot_digest: String,
    policy_digest: String,
    #[serde(deserialize_with = "required_nullable")]
    baseline_digest: Option<String>,
    advisory: bool,
    clean: bool,
}
fn required_nullable<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Option<String>, D::Error> {
    Option::<String>::deserialize(d)
}
impl CandidateSnapshot {
    pub fn advisory(&self) -> bool {
        self.advisory
    }
    pub fn task_id(&self) -> &str {
        &self.task_id
    }
    pub fn repo_id(&self) -> &str {
        &self.repo_id
    }
    pub fn worktree_id(&self) -> &str {
        &self.worktree_id
    }
    pub fn base_oid(&self) -> &str {
        &self.base_oid
    }
    pub fn merge_group_id(&self) -> Option<&str> {
        self.merge_group_id.as_deref()
    }
    pub fn members(&self) -> &[String] {
        &self.members
    }
    pub fn source_snapshot_digest(&self) -> &str {
        &self.source_snapshot_digest
    }
    pub fn policy_digest(&self) -> &str {
        &self.policy_digest
    }
    pub fn baseline_digest(&self) -> Option<&str> {
        self.baseline_digest.as_deref()
    }
    pub fn candidate_oid(&self) -> &str {
        &self.candidate_oid
    }
    pub fn clean(&self) -> bool {
        self.clean
    }
    pub fn requirement_ids(&self) -> &[String] {
        &self.requirement_ids
    }
    /// Canonical byte-sorted, deduplicated path prefixes bound into this snapshot.
    pub fn allowed_paths(&self) -> &[Vec<u8>] {
        &self.allowed_paths
    }
    pub fn binding_digest(&self) -> String {
        hash(&serde_json::to_vec(self).expect("string/vector serialization"))
    }
    pub fn validate(&self, repo: &Repository) -> Result<()> {
        if self.schema_version != "gitguard.candidate/v1alpha2"
            || !self.advisory
            || self.repo_id != repo.repo_id
            || self.object_format != repo.object_format()
            || self.task_id.is_empty()
            || self.worktree_id.is_empty()
            || self.requirement_ids.is_empty()
            || self.requirement_ids.iter().any(|s| s.is_empty())
            || self.requirement_ids.windows(2).any(|s| s[0] >= s[1])
            || self.allowed_paths.iter().any(|path| !path_valid(path))
            || self
                .allowed_paths
                .windows(2)
                .any(|paths| paths[0] >= paths[1])
            || !digest_valid(&self.source_snapshot_digest)
            || !digest_valid(&self.policy_digest)
            || self.baseline_digest.is_some()
            || self.merge_group_id.as_ref().is_some_and(|s| s.is_empty())
            || self.merge_group_id.is_some() != !self.members.is_empty()
        {
            return Err(Diagnostic::InvalidBinding);
        }
        repo.commit(&self.candidate_oid)?;
        repo.commit(&self.base_oid)?;
        let mut seen = std::collections::HashSet::new();
        for oid in &self.members {
            repo.commit(oid)?;
            if !seen.insert(oid) {
                return Err(Diagnostic::InvalidBinding);
            }
        }
        // Deserialized records must bind the actual candidate bytes, not only a plausible digest.
        let frozen = repo.resolve_subject(SubjectRequest::Commit(self.candidate_oid.clone()))?;
        if frozen.digest() != self.source_snapshot_digest || frozen.clean() != self.clean {
            return Err(Diagnostic::InvalidBinding);
        }
        Ok(())
    }
}
impl Repository {
    pub fn preview_merge(&self, base: &str, head: &str) -> Result<FrozenSubject> {
        self.commit(base)?;
        self.commit(head)?;
        let tree = self
            .run(&["merge-tree", "--write-tree", base, head])
            .map_err(|e| {
                if e == Diagnostic::GitFailed {
                    Diagnostic::Conflict
                } else {
                    e
                }
            })?;
        let tree = std::str::from_utf8(&tree)
            .map_err(|_| Diagnostic::GitFailed)?
            .trim();
        self.object(tree, "tree")?;
        let oid = self.run(&[
            "-c",
            "user.name=GitGuard Preview",
            "-c",
            "user.email=preview@example.invalid",
            "commit-tree",
            tree,
            "-p",
            base,
            "-p",
            head,
            "-m",
            "GitGuard advisory merge preview",
        ])?;
        let oid = std::str::from_utf8(&oid)
            .map_err(|_| Diagnostic::GitFailed)?
            .trim();
        self.resolve_subject(SubjectRequest::Commit(oid.into()))
    }
    pub fn prepare_candidate(
        &self,
        subject: &FrozenSubject,
        scope: &TaskScope,
        request: &CandidateRequest,
    ) -> Result<CandidateSnapshot> {
        scope.validate()?;
        if subject.repo_id != self.repo_id {
            return Err(Diagnostic::InvalidBinding);
        }
        let candidate = subject.candidate_oid().ok_or(Diagnostic::InvalidObject)?;
        let current = self.resolve_subject(SubjectRequest::Commit(candidate.into()))?;
        if current.digest() != subject.digest() {
            return Err(Diagnostic::InvalidBinding);
        }
        let mut allowed_paths = scope.allowed_paths.clone();
        allowed_paths.sort();
        allowed_paths.dedup();
        let snapshot = CandidateSnapshot {
            schema_version: "gitguard.candidate/v1alpha2".into(),
            repo_id: self.repo_id.clone(),
            task_id: scope.task_id.clone(),
            worktree_id: request.worktree_id.clone(),
            requirement_ids: scope.requirement_ids.clone(),
            allowed_paths,
            object_format: self.object_format().into(),
            candidate_oid: candidate.into(),
            base_oid: request.base_oid.clone(),
            merge_group_id: request.merge_group_id.clone(),
            members: request.members.clone(),
            source_snapshot_digest: subject.digest.clone(),
            policy_digest: scope.policy_digest.clone(),
            baseline_digest: scope.baseline_digest.clone(),
            advisory: true,
            clean: current.clean,
        };
        snapshot.validate(self)?;
        Ok(snapshot)
    }
}
