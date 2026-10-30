//! Bounded GitHub merge-group webhook observation; not hosted admission.
use crate::{
    Repository,
    candidate::{CandidateRequest, CandidateSnapshot},
    scope::TaskScope,
    subject::SubjectRequest,
};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::collections::BTreeSet;
pub const MAX_BODY: usize = 1024 * 1024;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Signature,
    Configuration,
    Budget,
    Schema,
    Binding,
    Mapping,
    Replay,
    Unobserved,
    Unavailable,
}
/// Controller-provisioned webhook secret. Never serialized or exposed through Debug.
pub struct WebhookKey {
    mac: Hmac<Sha256>,
}
impl WebhookKey {
    pub fn new(secret: &[u8]) -> Result<Self, Error> {
        if !(16..=4096).contains(&secret.len()) {
            return Err(Error::Configuration);
        }
        Ok(Self {
            mac: Hmac::<Sha256>::new_from_slice(secret).map_err(|_| Error::Configuration)?,
        })
    }
    pub fn verify_signature(&self, raw: &[u8], signature: &str) -> Result<(), Error> {
        if raw.len() > MAX_BODY {
            return Err(Error::Budget);
        }
        let hex = signature.strip_prefix("sha256=").ok_or(Error::Signature)?;
        if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Error::Signature);
        }
        let mut tag = [0u8; 32];
        for (i, b) in tag.iter_mut().enumerate() {
            *b = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).map_err(|_| Error::Signature)?;
        }
        let mut mac = self.mac.clone();
        mac.update(raw);
        mac.verify_slice(&tag).map_err(|_| Error::Signature)
    }
}
/// Trusted controller configuration; upstream optional repository/installation are required here.
pub struct Config {
    pub repository_id: u64,
    pub installation_id: u64,
    pub repo_id: String,
    pub base_refs: BTreeSet<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct GroupIdentity {
    pub repository_id: u64,
    pub installation_id: u64,
    pub head_sha: String,
    pub base_sha: String,
    pub head_ref: String,
    pub base_ref: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    ChecksRequested,
    Destroyed,
}
/// Only exact-body HMAC verification and decoding can construct this value.
pub struct VerifiedMergeGroup {
    identity: GroupIdentity,
    repo_id: String,
    action: Action,
    body_digest: String,
    tree_id: String,
}
impl VerifiedMergeGroup {
    pub fn identity(&self) -> &GroupIdentity {
        &self.identity
    }
    pub fn action(&self) -> Action {
        self.action
    }
    pub fn body_digest(&self) -> &str {
        &self.body_digest
    }
}
fn text(value: &serde_json::Value, max: usize) -> Result<&str, Error> {
    value
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= max)
        .ok_or(Error::Schema)
}
fn oid(s: &str) -> bool {
    s.len() == 40
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn label(s: &str) -> bool {
    !s.is_empty() && s.len() <= 256 && !s.chars().any(char::is_control)
}
fn full_ref(s: &str) -> bool {
    s.starts_with("refs/heads/")
        && s.len() <= 1024
        && !s.contains("..")
        && !s.contains("@{")
        && !s
            .bytes()
            .any(|b| b <= 32 || b == 127 || b"~^:?*[\\".contains(&b))
        && s.split('/').all(|p| {
            !p.is_empty() && !p.starts_with('.') && !p.ends_with('.') && !p.ends_with(".lock")
        })
}
/// GitHub.com merge_group profile, fixed to the retained official schema subset.
/// HMAC covers only body bytes; event/delivery headers and currentness are separate.
pub fn verify(
    config: &Config,
    key: &WebhookKey,
    raw: &[u8],
    event: &str,
    signature: &str,
) -> Result<VerifiedMergeGroup, Error> {
    if config.repository_id == 0
        || config.installation_id == 0
        || !label(&config.repo_id)
        || config.base_refs.is_empty()
        || config.base_refs.len() > 32
        || config.base_refs.iter().any(|s| !full_ref(s))
    {
        return Err(Error::Configuration);
    }
    key.verify_signature(raw, signature)?;
    if event != "merge_group" {
        return Err(Error::Schema);
    }
    let v = crate::api::unique_json(raw).map_err(|_| Error::Schema)?;
    let action = match v["action"].as_str() {
        Some("checks_requested") => Action::ChecksRequested,
        Some("destroyed") => Action::Destroyed,
        _ => return Err(Error::Schema),
    };
    if let Some(reason) = v.get("reason")
        && !matches!(reason.as_str(), Some("merged" | "invalidated" | "dequeued"))
    {
        return Err(Error::Schema);
    }
    if v["repository"]["id"].as_u64() != Some(config.repository_id)
        || v["installation"]["id"].as_u64() != Some(config.installation_id)
    {
        return Err(Error::Binding);
    }
    text(&v["installation"]["node_id"], 1024)?;
    let g = &v["merge_group"];
    let head = text(&g["head_sha"], 40)?;
    let base = text(&g["base_sha"], 40)?;
    let head_ref = text(&g["head_ref"], 1024)?;
    let base_ref = text(&g["base_ref"], 1024)?;
    if !oid(head)
        || !oid(base)
        || !full_ref(head_ref)
        || !full_ref(base_ref)
        || !config.base_refs.contains(base_ref)
    {
        return Err(Error::Binding);
    }
    let commit = &g["head_commit"];
    if text(&commit["id"], 40)? != head {
        return Err(Error::Binding);
    }
    let tree = text(&commit["tree_id"], 40)?;
    if !oid(tree) {
        return Err(Error::Schema);
    }
    // Metadata shape only. The timestamp is never a freshness/authority clock.
    if !commit["message"].is_string() {
        return Err(Error::Schema);
    }
    text(&commit["timestamp"], 128)?;
    for who in ["author", "committer"] {
        let person = commit.get(who).ok_or(Error::Schema)?;
        if !person.is_null() {
            text(&person["name"], 1024)?;
            text(&person["email"], 1024)?;
        }
    }
    Ok(VerifiedMergeGroup {
        identity: GroupIdentity {
            repository_id: config.repository_id,
            installation_id: config.installation_id,
            head_sha: head.into(),
            base_sha: base.into(),
            head_ref: head_ref.into(),
            base_ref: base_ref.into(),
        },
        repo_id: config.repo_id.clone(),
        action,
        body_digest: format!("sha256:{}", crate::subject::hash(raw)),
        tree_id: tree.into(),
    })
}
/// Caller-owned current lookup and mapping. These fields do not exist in GitHub payloads.
#[derive(Clone)]
pub struct ControllerCurrent {
    pub identity: GroupIdentity,
    pub group_id: String,
    pub generation: u64,
}
pub struct Mapping {
    pub current: ControllerCurrent,
    pub scope: TaskScope,
    pub worktree_id: String,
    pub members: Vec<String>,
}
pub struct MappedCandidate {
    candidate: CandidateSnapshot,
    current: ControllerCurrent,
    body_digest: String,
}
impl MappedCandidate {
    pub fn candidate(&self) -> &CandidateSnapshot {
        &self.candidate
    }
}
/// Membership/task scope come from the protected controller, never ref-name parsing.
/// The result remains advisory; it does not grant execution or assert currentness.
pub fn map_candidate(
    event: &VerifiedMergeGroup,
    repo: &Repository,
    mapping: &Mapping,
) -> Result<MappedCandidate, Error> {
    crate::evidence::freshness::admit(&mapping.scope, 1024 * 1024).map_err(|_| Error::Budget)?;
    if event.action != Action::ChecksRequested
        || repo.repo_id() != event.repo_id
        || repo.object_format() != "sha1"
        || mapping.current.identity != event.identity
        || mapping.current.generation == 0
        || !label(&mapping.current.group_id)
        || !label(&mapping.worktree_id)
        || mapping.members.is_empty()
        || mapping.members.len() > 128
        || mapping.members.iter().any(|s| !oid(s))
    {
        return Err(Error::Mapping);
    }
    let subject = repo
        .resolve_subject(SubjectRequest::Commit(event.identity.head_sha.clone()))
        .map_err(|_| Error::Mapping)?;
    let tree = repo
        .run(&[
            "rev-parse",
            &format!("{}^{{tree}}", event.identity.head_sha),
        ])
        .map_err(|_| Error::Mapping)?;
    if tree != format!("{}\n", event.tree_id).as_bytes() {
        return Err(Error::Mapping);
    }
    let candidate = repo
        .prepare_candidate(
            &subject,
            &mapping.scope,
            &CandidateRequest {
                worktree_id: mapping.worktree_id.clone(),
                base_oid: event.identity.base_sha.clone(),
                merge_group_id: Some(mapping.current.group_id.clone()),
                members: mapping.members.clone(),
            },
        )
        .map_err(|_| Error::Mapping)?;
    Ok(MappedCandidate {
        candidate,
        current: mapping.current.clone(),
        body_digest: event.body_digest.clone(),
    })
}
/// A protected caller supplies an independently authenticated current lookup, queried each time.
/// No network client or default successful implementation is supplied by this adapter.
pub trait CurrentLookup {
    fn current(
        &self,
        repository_id: u64,
        installation_id: u64,
        base_ref: &str,
    ) -> Result<Option<ControllerCurrent>, Error>;
}
/// Bounded process-local replay/withdrawal memory. Persistence is controller-owned.
/// Recording an event never selects the current group; only CurrentLookup can do that.
#[derive(Default)]
pub struct ReplayGuard {
    bodies: BTreeSet<String>,
    deliveries: BTreeSet<String>,
    withdrawn: BTreeSet<GroupIdentity>,
}
impl ReplayGuard {
    pub fn record(&mut self, event: &VerifiedMergeGroup, delivery: &str) -> Result<(), Error> {
        if !label(delivery) || delivery.len() > 128 {
            return Err(Error::Schema);
        }
        if self.bodies.contains(&event.body_digest) || self.deliveries.contains(delivery) {
            return Err(Error::Replay);
        }
        if self.bodies.len() >= 256 {
            return Err(Error::Budget);
        }
        self.bodies.insert(event.body_digest.clone());
        self.deliveries.insert(delivery.into());
        if event.action == Action::Destroyed {
            self.withdrawn.insert(event.identity.clone());
        }
        Ok(())
    }
    pub fn is_current(
        &self,
        mapped: &MappedCandidate,
        lookup: &dyn CurrentLookup,
    ) -> Result<bool, Error> {
        if !self.bodies.contains(&mapped.body_digest) {
            return Err(Error::Unobserved);
        }
        let expected = &mapped.current;
        if self.withdrawn.contains(&expected.identity) {
            return Ok(false);
        }
        let actual = lookup.current(
            expected.identity.repository_id,
            expected.identity.installation_id,
            &expected.identity.base_ref,
        )?;
        Ok(actual.is_some_and(|v| {
            v.identity == expected.identity
                && v.group_id == expected.group_id
                && v.generation == expected.generation
        }))
    }
}
