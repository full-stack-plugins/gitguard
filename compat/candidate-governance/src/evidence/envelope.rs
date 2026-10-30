use crate::{
    Repository,
    cli::CheckRequest,
    evidence::projection::{
        ANALYZER_ID, ANALYZER_VERSION, FrozenPolicy, MAPPING_VERSION, evaluate_projected, project,
    },
    preflight::{PreflightResult, preflight},
    subject::{SubjectRequest, hash},
};
use guardengine::{
    Completeness, Decision,
    integration::{
        self, ArtifactRef, Artifacts, AttemptOutput, BoundAttempt, Coverage, CoverageStatus,
        Diagnostic, EvidenceProfile, GuardRunEnvelope, InvocationDraft, Producer, RunBinding,
        RunStatus,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
pub const CHECK_VERSION: &str = "gitguard.check/v1alpha1";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckBundle {
    pub api_version: String,
    pub envelope: GuardRunEnvelope,
    pub contract: Option<Value>,
    pub facts: Option<Value>,
    pub report: Option<Value>,
    pub domain: Value,
}
impl CheckBundle {
    pub fn exit_code(&self) -> i32 {
        match (&self.envelope.run_status, &self.envelope.decision) {
            (RunStatus::Completed, Some(Decision::Allow)) => 0,
            (RunStatus::Completed, Some(Decision::Block)) => 2,
            (RunStatus::Completed, Some(Decision::RequireApproval)) => 3,
            _ => 4,
        }
    }
}
pub struct BoundCheck {
    repo: Repository,
    result: PreflightResult,
    policy: FrozenPolicy,
    attempt: BoundAttempt,
    coverage: Coverage,
}
fn now() -> Result<String, String> {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| "clock unavailable".into())
}
fn artifact(name: &str, value: &Value) -> ArtifactRef {
    let digest = hash(&serde_json::to_vec(value).expect("JSON value serialization"));
    ArtifactRef {
        uri: format!("artifact://gitguard/{name}/{digest}"),
        digest: format!("sha256:{digest}"),
        media_type: "application/json".into(),
    }
}
impl BoundCheck {
    pub fn prepare(request: CheckRequest) -> Result<Self, String> {
        if request.api_version != CHECK_VERSION {
            return Err("unsupported check version".into());
        }
        let policy = FrozenPolicy::from_contract(request.contract)?;
        if request.scope.policy_digest != policy.digest() {
            return Err("frozen policy digest mismatch".into());
        }
        let repo = Repository::discover(&request.repo_root, &request.repo_id)
            .map_err(|_| "repository discovery failed")?;
        let result = preflight(
            &repo,
            SubjectRequest::Commit(request.candidate_oid),
            &request.scope,
            &request.candidate,
        )
        .map_err(|_| "candidate preflight failed")?;
        let c = &result.candidate;
        let mut required = vec![
            format!("git.scope:{}", c.binding_digest()),
            "git.source-clean".into(),
            "git.submodule-contents".into(),
        ];
        required.sort();
        let observed = required
            .iter()
            .filter(|s| {
                !(s.as_str() == "git.source-clean" && !c.clean()
                    || s.as_str() == "git.submodule-contents"
                        && !result.changes.missing_coverage.is_empty())
            })
            .cloned()
            .collect::<Vec<_>>();
        let missing = required
            .iter()
            .filter(|s| !observed.contains(s))
            .cloned()
            .collect::<Vec<_>>();
        let binding = RunBinding {
            repo_id: c.repo_id().into(),
            task_id: c.task_id().into(),
            worktree_id: c.worktree_id().into(),
            requirement_ids: c.requirement_ids().to_vec(),
            candidate_oid: c.candidate_oid().into(),
            base_oid: c.base_oid().into(),
            merge_group_id: c.merge_group_id().map(str::to_owned),
            source_snapshot_digest: format!("sha256:{}", c.source_snapshot_digest()),
            baseline_digest: None,
        };
        let nonce = tempfile::tempdir().map_err(|_| "run identity unavailable")?;
        let run_id = hash(nonce.path().to_string_lossy().as_bytes());
        let coverage = Coverage {
            status: if missing.is_empty() {
                CoverageStatus::Complete
            } else {
                CoverageStatus::Partial
            },
            required_scopes: required,
            observed_scopes: observed,
            missing_scopes: missing,
        };
        let attempt = integration::prepare_attempt(InvocationDraft {
            run_id,
            producer: Some(Producer {
                guard: "GitGuard".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                analyzer_id: ANALYZER_ID.into(),
                analyzer_version: ANALYZER_VERSION.into(),
            }),
            binding: Some(binding),
            coverage: Some(coverage.clone()),
            profile: Some(EvidenceProfile::EngineBacked),
            started_at: now()?,
        })
        .map_err(|_| "attempt binding invalid")?;
        Ok(Self {
            repo,
            result,
            policy,
            attempt,
            coverage,
        })
    }
    pub fn run(self, cancel: &AtomicBool) -> Result<CheckBundle, String> {
        let domain = serde_json::json!({"apiVersion":"gitguard.domain/v1alpha1","mappingVersion":MAPPING_VERSION,"candidate":self.result.candidate,"changes":self.result.changes,"advisory":true,"authorization":"not-evaluated"});
        let mut artifacts = Artifacts {
            contract: None,
            facts: None,
            report: None,
            domain: vec![artifact("domain", &domain)],
        };
        let mut contract = None;
        let mut facts = None;
        let mut report = None;
        let projection = if cancel.load(Ordering::SeqCst) {
            None
        } else {
            Some(
                project(&self.repo, &self.result, &self.policy).and_then(|facts| {
                    let report = evaluate_projected(&self.policy, &facts)?;
                    Ok((facts, report))
                }),
            )
        };
        let (run_status, decision, diagnostics) = if cancel.load(Ordering::SeqCst) {
            (
                RunStatus::Cancelled,
                None,
                vec![Diagnostic {
                    code: "run.cancelled".into(),
                    message: "check cancelled".into(),
                    retryable: true,
                    source: None,
                }],
            )
        } else {
            match projection.unwrap_or_else(|| Err("cancelled".into())) {
                Ok((fact_set, evaluated)) => {
                    let engine_contract = self.policy.contract();
                    let decision = evaluated.decision.clone();
                    let diagnostic = if fact_set.completeness == Completeness::Partial {
                        vec![Diagnostic {
                            code: "coverage.partial".into(),
                            message: "source or submodule coverage incomplete".into(),
                            retryable: true,
                            source: None,
                        }]
                    } else {
                        vec![]
                    };
                    let cv = serde_json::to_value(engine_contract)
                        .map_err(|_| "serialization failed")?;
                    let fv = serde_json::to_value(fact_set).map_err(|_| "serialization failed")?;
                    let rv = serde_json::to_value(evaluated).map_err(|_| "serialization failed")?;
                    artifacts.contract = Some(artifact("contract", &cv));
                    artifacts.facts = Some(artifact("facts", &fv));
                    artifacts.report = Some(artifact("report", &rv));
                    contract = Some(cv);
                    facts = Some(fv);
                    report = Some(rv);
                    (RunStatus::Completed, Some(decision), diagnostic)
                }
                Err(_) => (
                    RunStatus::Error,
                    None,
                    vec![Diagnostic {
                        code: "observation.failed".into(),
                        message: "bound source observation failed".into(),
                        retryable: true,
                        source: None,
                    }],
                ),
            }
        };
        // A failed/cancelled re-observation cannot publish prior coverage as current success.
        let mut final_coverage = self.coverage;
        if run_status != RunStatus::Completed {
            final_coverage.status = CoverageStatus::Partial;
            final_coverage.observed_scopes.clear();
            final_coverage.missing_scopes = final_coverage.required_scopes.clone();
        }
        let envelope = self
            .attempt
            .finish(AttemptOutput {
                coverage: final_coverage,
                run_status,
                decision,
                artifacts,
                approval_refs: vec![],
                diagnostics,
                finished_at: now()?,
                expires_at: None,
            })
            .map_err(|_| "attempt outcome invalid")?;
        if let (Some(c), Some(f), Some(r)) = (&contract, &facts, &report) {
            integration::verify_engine_artifacts(
                &envelope,
                &serde_json::to_vec(c).map_err(|_| "serialization failed")?,
                &serde_json::to_vec(f).map_err(|_| "serialization failed")?,
                &serde_json::to_vec(r).map_err(|_| "serialization failed")?,
            )
            .map_err(|_| "engine artifact verification failed")?;
        }
        Ok(CheckBundle {
            api_version: CHECK_VERSION.into(),
            envelope,
            contract,
            facts,
            report,
            domain,
        })
    }
}
