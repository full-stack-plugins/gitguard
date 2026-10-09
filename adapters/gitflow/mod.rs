//! Opt-in, read-only import of untrusted Gitflow compatibility observations.
//! No process execution, authority/GE envelope, or default CLI routing is installed.
use crate::{Repository, candidate::CandidateSnapshot};
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub const PROFILE: &str = "gitflow/310c43c5e3fe7effa19245dcc4970249e569004c/check-commits";
const LIMIT: usize = 1024 * 1024;
#[derive(Serialize)]
pub struct ExpectedWorkflow {
    pub profile: String,
    pub source: String,
    pub target: String,
    pub policy_ref: String,
    pub raw_policy_digest: String,
    /// Independently controller-declared normalized-policy hash, not producer identity.
    pub native_policy_digest: String,
}
#[derive(Debug, PartialEq, Eq)]
pub enum ImportError {
    Budget,
    Invalid,
    Binding,
    NativeUsage,
    NativeInternal,
    Timeout,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Compatible,
    Denied,
    Incomplete,
}
pub enum NativeOutput<'a> {
    Exited { code: i32, stdout: &'a [u8] },
    Timeout,
}
/// Historical local observation only. Compatible cannot satisfy a mandatory GE gate.
pub struct Observation {
    outcome: Outcome,
    raw_digest: String,
    expectation_digest: String,
}
impl Observation {
    pub fn outcome(&self) -> Outcome {
        self.outcome
    }
    pub fn raw_digest(&self) -> &str {
        &self.raw_digest
    }
    pub fn expectation_digest(&self) -> &str {
        &self.expectation_digest
    }
}
pub struct PreparedCheck {
    expected: ExpectedWorkflow,
    binding: String,
    head: String,
    commits: Vec<String>,
    digest: String,
}
fn budget<T: Serialize>(v: &T) -> Result<(), ImportError> {
    crate::evidence::freshness::admit(v, LIMIT).map_err(|_| ImportError::Budget)
}
fn branch(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 256
        && !s.contains("..")
        && s.split('/').all(|x| {
            !x.is_empty()
                && !x.starts_with('.')
                && !x.ends_with('.')
                && !x.ends_with(".lock")
                && x.bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
        })
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Report {
    schema_version: String,
    action: String,
    decision: String,
    reasons: Vec<Value>,
    next_actions: Vec<Value>,
    base: Option<String>,
    head: Option<String>,
    source: Option<String>,
    target: Option<String>,
    policy_ref: Option<String>,
    policy_sha256: Option<String>,
    revision: Option<u64>,
    commit_count: Option<usize>,
    commits: Option<Vec<Commit>>,
    message_mode: Option<String>,
    squash_checks: Option<Vec<Value>>,
    tag_checks: Option<Vec<Value>>,
    branch_origin: Option<String>,
    server_protection: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Commit {
    oid: String,
    decision: String,
    checks: Vec<Value>,
    reasons: Vec<Value>,
}
fn checks_allow(checks: &[Value]) -> bool {
    let expected = [
        "GF001", "GF002", "GF101", "GF102", "GF103", "GF104", "GF401",
    ];
    let ids: std::collections::BTreeSet<_> = checks
        .iter()
        .filter_map(|v| v["rule_id"].as_str())
        .collect();
    checks.len() == expected.len()
        && ids == expected.into_iter().collect()
        && checks.iter().all(|v| {
            let Some(m) = v.as_object() else {
                return false;
            };
            m.len() == 9
                && [
                    "rule_id",
                    "name",
                    "severity",
                    "status",
                    "actual",
                    "expected",
                    "message",
                    "suggestion",
                    "fix",
                ]
                .iter()
                .all(|k| m.contains_key(*k))
                && m["rule_id"].is_string()
                && m["name"].is_string()
                && m["message"].is_string()
                && matches!(m["severity"].as_str(), Some("off" | "warn" | "error"))
                && match m["status"].as_str() {
                    Some("pass") => true,
                    Some("skipped") => m["severity"] == "off",
                    Some("fail") => m["severity"] == "warn" || m["severity"] == "off",
                    _ => false,
                }
        })
}
impl PreparedCheck {
    pub fn freeze(
        repo: &Repository,
        candidate: &CandidateSnapshot,
        expected: ExpectedWorkflow,
        raw_policy: &[u8],
    ) -> Result<Self, ImportError> {
        if raw_policy.len() > LIMIT {
            return Err(ImportError::Budget);
        }
        budget(&(candidate, &expected))?;
        if expected.profile != PROFILE
            || !branch(&expected.source)
            || !branch(&expected.target)
            || expected.policy_ref != candidate.base_oid()
            || !crate::scope::digest_valid(&expected.native_policy_digest)
            || crate::subject::hash(raw_policy) != expected.raw_policy_digest
        {
            return Err(ImportError::Binding);
        }
        candidate.validate(repo).map_err(|_| ImportError::Binding)?;
        let tree = repo
            .run(&[
                "ls-tree",
                &expected.policy_ref,
                "--",
                ".gitflow/workflow.json",
            ])
            .map_err(|_| ImportError::Binding)?;
        if !tree.starts_with(b"100644 blob ") && !tree.starts_with(b"100755 blob ") {
            return Err(ImportError::Binding);
        }
        let actual = repo
            .run(&[
                "show",
                &format!("{}:.gitflow/workflow.json", expected.policy_ref),
            ])
            .map_err(|_| ImportError::Binding)?;
        if actual != raw_policy {
            return Err(ImportError::Binding);
        }
        let range = repo
            .run(&[
                "rev-list",
                "--reverse",
                "--max-count=501",
                &format!("{}..{}", candidate.base_oid(), candidate.candidate_oid()),
            ])
            .map_err(|_| ImportError::Binding)?;
        let commits: Vec<String> = std::str::from_utf8(&range)
            .map_err(|_| ImportError::Binding)?
            .lines()
            .map(str::to_owned)
            .collect();
        if commits.len() > 500 {
            return Err(ImportError::Budget);
        }
        let binding = candidate.binding_digest();
        let digest = guardengine::digest_json(&(PROFILE, &binding, &expected, &commits))
            .map_err(|_| ImportError::Invalid)?;
        Ok(Self {
            expected,
            binding,
            head: candidate.candidate_oid().into(),
            commits,
            digest,
        })
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn consume(
        &self,
        candidate: &CandidateSnapshot,
        output: NativeOutput<'_>,
    ) -> Result<Observation, ImportError> {
        budget(candidate)?;
        if candidate.binding_digest() != self.binding {
            return Err(ImportError::Binding);
        }
        let (code, raw) = match output {
            NativeOutput::Timeout => return Err(ImportError::Timeout),
            NativeOutput::Exited { code, stdout } => (code, stdout),
        };
        if raw.len() > LIMIT {
            return Err(ImportError::Budget);
        }
        let value = crate::api::unique_json(raw).map_err(|_| ImportError::Invalid)?;
        let r: Report = serde_json::from_value(value).map_err(|_| ImportError::Invalid)?;
        if r.schema_version != "1.0.0" {
            return Err(ImportError::Invalid);
        }
        match (code, r.action.as_str(), r.decision.as_str()) {
            (2, "usage", "error") => return Err(ImportError::NativeUsage),
            (4, "check", "error") => return Err(ImportError::NativeInternal),
            (0, "check", "allow") | (1, "check", "deny") | (3, "check", "unverified") => {}
            _ => return Err(ImportError::Invalid),
        }
        let outcome = if code == 3 {
            Outcome::Incomplete
        } else {
            if r.base.as_deref() != Some(&self.expected.policy_ref)
                || r.head.as_deref() != Some(&self.head)
                || r.source.as_deref() != Some(&self.expected.source)
                || r.target.as_deref() != Some(&self.expected.target)
                || r.policy_ref.as_deref() != Some(&self.expected.policy_ref)
                || r.policy_sha256.as_deref() != Some(&self.expected.native_policy_digest)
                || r.revision.is_none_or(|n| n == 0)
                || r.message_mode.as_deref() != Some("commits")
                || r.branch_origin.as_deref() != Some("not_inferred")
                || r.server_protection.as_deref() != Some("unverified")
            {
                return Err(ImportError::Binding);
            }
            let commits = r.commits.ok_or(ImportError::Invalid)?;
            if r.commit_count != Some(self.commits.len())
                || commits.len() != self.commits.len()
                || commits.iter().zip(&self.commits).any(|(a, b)| &a.oid != b)
            {
                return Err(ImportError::Binding);
            }
            let squash = r.squash_checks.ok_or(ImportError::Invalid)?;
            let tags = r.tag_checks.ok_or(ImportError::Invalid)?;
            if code == 0 {
                if !r.reasons.is_empty()
                    || !r.next_actions.is_empty()
                    || !squash.is_empty()
                    || !checks_allow(&tags)
                    || commits.iter().any(|c| {
                        c.decision != "allow" || !c.reasons.is_empty() || !checks_allow(&c.checks)
                    })
                {
                    return Err(ImportError::Invalid);
                }
                Outcome::Compatible
            } else {
                Outcome::Denied
            }
        };
        Ok(Observation {
            outcome,
            raw_digest: crate::subject::hash(raw),
            expectation_digest: self.digest.clone(),
        })
    }
}
