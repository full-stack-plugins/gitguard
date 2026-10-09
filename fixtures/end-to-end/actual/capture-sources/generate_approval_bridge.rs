mod approval_bridge_support;
// Reproducible local fixture generator. Does not install tools or grant production authority.
use archguard::integration::{binding::GitCargoEvidence, projection::ProtectedCargoPolicy};
use flowguard::{context::*, dependencies::*, evidence::*, gate::obligation_scope, obligations::*};
use gitguard::{
    Repository,
    candidate::{CandidateRequest, CandidateSnapshot},
    cli::CheckRequest,
    evidence::{envelope::BoundCheck, projection::FrozenPolicy},
    scope::TaskScope,
    subject::SubjectRequest,
};
use guardengine::{
    integration::{Producer, RunBinding, eligibility::EligibilityPolicy},
    *,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::atomic::AtomicBool,
};
#[derive(Serialize, Deserialize)]
struct Expected {
    binding: RunBinding,
    producer: Producer,
    required_scopes: Vec<String>,
    contract_digest: String,
}
fn write(p: impl AsRef<Path>, v: &impl Serialize) {
    std::fs::write(p, serde_json::to_vec(v).unwrap()).unwrap();
}
fn read<T: serde::de::DeserializeOwned>(p: impl AsRef<Path>) -> T {
    serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap()
}
fn contract() -> GuardContract {
    serde_json::from_value(serde_json::json!({"apiVersion":API_VERSION,"kind":"GuardContract","metadata":{"id":"cross-guard-architecture","revision":"1"},"spec":{"rules":[{"id":"direction","enforcement":"enforce","assertion":{"type":"forbid_relation","subject":"cross-guard-core","predicate":"depends_on","object":"cross-guard-app"}}]}})).unwrap()
}
fn request(candidate: &CandidateSnapshot) -> CandidateRequest {
    CandidateRequest {
        worktree_id: candidate.worktree_id().into(),
        base_oid: candidate.base_oid().into(),
        merge_group_id: None,
        members: vec![],
    }
}
fn binding(repo: &Repository, c: &CandidateSnapshot) -> ValidatedBinding {
    bind(
        &InvocationInput {
            repo_candidates: vec![c.repo_id().into()],
            task_candidates: vec![c.task_id().into()],
            worktree_id: c.worktree_id().into(),
            requirement_ids: c.requirement_ids().to_vec(),
            candidate_oid: c.candidate_oid().into(),
            base_oid: c.base_oid().into(),
        },
        repo,
        c,
    )
    .unwrap()
}
fn main() {
    let a: Vec<_> = std::env::args().collect();
    assert_eq!(a.len(), 4, "MODE REPO OUTPUT");
    let mode = &a[1];
    let root = Path::new(&a[2]);
    let out = Path::new(&a[3]);
    let repo = Repository::discover(root, "cross-guard-repo").unwrap();
    if mode.starts_with("sg-") {
        approval_bridge_support::run(mode,root,out);
    } else if mode == "prepare" {
        for p in ["archguard", "gitguard", "testguard", "codeguard"] {
            std::fs::create_dir_all(out.join(p)).unwrap();
        }
        let oid = String::from_utf8(
            std::process::Command::new("/usr/bin/git")
                .arg("-C")
                .arg(root)
                .args(["rev-parse", "HEAD"])
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap()
        .trim()
        .to_owned();
        let subject = repo
            .resolve_subject(SubjectRequest::Commit(oid.clone()))
            .unwrap();
        let paths = vec![
            b"app".to_vec(),
            b"core".to_vec(),
            b"Cargo.toml".to_vec(),
            b"Cargo.lock".to_vec(),
            b"app.py".to_vec(),
            b"pyproject.toml".to_vec(),
            b"baseline.json".to_vec(),
            b".gitignore".to_vec(),
            b"specs".to_vec(),
        ];
        let cr = CandidateRequest {
            worktree_id: "fixture-worktree".into(),
            base_oid: std::env::var("APPROVAL_BASE_OID").unwrap(),
            merge_group_id: None,
            members: vec![],
        };
        let gp = FrozenPolicy::new(Enforcement::Enforce);
        let gs = TaskScope::advisory(
            "cross-guard-task",
            vec!["demo:R".into()],
            paths.clone(),
            &gp.digest(),
            None,
        )
        .unwrap();
        let gc = repo.prepare_candidate(&subject, &gs, &cr).unwrap();
        write(out.join("gitguard/candidate.json"), &gc);
        let b = binding(&repo, &gc);
        write(out.join("binding.json"), b.binding());
        let q = CheckRequest {
            api_version: "gitguard.check/v1alpha1".into(),
            repo_root: root.into(),
            repo_id: "cross-guard-repo".into(),
            candidate_oid: oid,
            scope: gs,
            candidate: cr,
            contract: gp.contract(),
        };
        write(out.join("gitguard/request.json"), &q);
        let mut scopes = vec![
            format!("git.scope:{}", gc.binding_digest()),
            "git.source-clean".into(),
            "git.submodule-contents".into(),
        ];
        scopes.sort();
        let cv = serde_json::to_value(gp.contract()).unwrap();
        write(out.join("gitguard/prepared-contract.json"), &cv);
        write(
            out.join("gitguard/expected.json"),
            &Expected {
                binding: b.binding().clone(),
                producer: Producer {
                    guard: "GitGuard".into(),
                    version: "0.1.0".into(),
                    analyzer_id: gitguard::evidence::projection::ANALYZER_ID.into(),
                    analyzer_version: gitguard::evidence::projection::ANALYZER_VERSION.into(),
                },
                required_scopes: scopes,
                contract_digest: flowguard::digest(&serde_json::to_vec(&cv).unwrap()),
            },
        );
        let ac = contract();
        write(out.join("archguard/policy.json"), &ac);
        let ap = ProtectedCargoPolicy::freeze(ac.clone(), vec![]).unwrap();
        let profile = ap.profile_digest().to_owned();
        let scope = TaskScope::advisory(
            "cross-guard-task",
            vec!["demo:R".into()],
            paths,
            profile.strip_prefix("sha256:").unwrap(),
            None,
        )
        .unwrap();
        let candidate = repo
            .prepare_candidate(&subject, &scope, &request(&gc))
            .unwrap();
        write(out.join("archguard/candidate.json"), &candidate);
        let prepared = GitCargoEvidence::prepare(&repo, &candidate, ap).unwrap();
        let scopes = BTreeSet::from([
            "cargo.declarations".into(),
            format!("cargo.profile:{profile}"),
            format!("cargo.member:{}", flowguard::digest(b"cross-guard-app")),
            format!("cargo.member:{}", flowguard::digest(b"cross-guard-core")),
            format!("cargo.relation:{}", flowguard::digest(b"depends_on")),
        ]);
        let cv = serde_json::to_value(&ac).unwrap();
        write(out.join("archguard/prepared-contract.json"), &cv);
        write(
            out.join("archguard/expected.json"),
            &Expected {
                binding: prepared.binding().clone(),
                producer: Producer {
                    guard: "ArchGuard".into(),
                    version: "0.1.0".into(),
                    analyzer_id: "archguard.cargo.declarations".into(),
                    analyzer_version: "0.1.0".into(),
                },
                required_scopes: scopes.into_iter().collect(),
                contract_digest: flowguard::digest(&serde_json::to_vec(&cv).unwrap()),
            },
        );
    } else if mode == "cg-prepare" {
        let wire: serde_json::Value = read(out.join("codeguard/prepare.stdout.json"));
        let envelope: guardengine::integration::GuardRunEnvelope =
            serde_json::from_value(wire["envelope"].clone()).unwrap();
        assert_eq!(
            envelope.run_status,
            guardengine::integration::RunStatus::Error
        );
        assert!(envelope.decision.is_none());
        let contract: GuardContract = read(out.join("codeguard/contract-input.json"));
        let raw = serde_json::to_vec(&contract).unwrap();
        std::fs::write(out.join("codeguard/prepared-contract.json"), &raw).unwrap();
        write(
            out.join("codeguard/expected.json"),
            &Expected {
                binding: envelope.binding,
                producer: envelope.producer,
                required_scopes: envelope.coverage.required_scopes,
                contract_digest: flowguard::digest(&raw),
            },
        );
    } else if mode == "freeze" {
        let candidate: CandidateSnapshot = read(out.join("gitguard/candidate.json"));
        let b = binding(&repo, &candidate);
        let graph = build_graph(
            vec![StageRecord {
                version: "flowguard.workflow/v1".into(),
                id: "release".into(),
                stage: "10-release".into(),
                owner: "project".into(),
                source_digest: flowguard::digest(b"protected cross-guard release requirements"),
                dependencies: BTreeSet::new(),
            }],
            GraphLimits::default(),
        )
        .unwrap();
        let baseline = flowguard::digest(&std::fs::read(root.join("baseline.json")).unwrap());
        let mut observations = vec![];
        for name in ["archguard", "codeguard", "testguard", "gitguard", "specguard"] {
            let e: Expected = read(out.join(name).join("expected.json"));
            observations.push((name, e));
        }
        let obligations = observations
            .iter()
            .map(|(_, e)| EvidenceObligation {
                stage_id: "release".into(),
                guard: e.producer.guard.clone(),
                coverage: e.required_scopes.iter().cloned().collect(),
                rules_digest: e.contract_digest.clone(),
                analyzer_version: e.producer.analyzer_version.clone(),
            })
            .collect();
        let frozen = freeze(&graph, obligations, &b.domain_digest(), &baseline).unwrap();
        let mut policies = BTreeMap::new();
        let mut source = BTreeMap::new();
        let mut names = BTreeMap::new();
        for (name, e) in observations {
            let o = frozen
                .obligations()
                .iter()
                .find(|o| o.guard == e.producer.guard)
                .unwrap();
            let scope = obligation_scope(o);
            let baseline = if name == "testguard" || name == "specguard" {
                assert_eq!(
                    e.binding.baseline_digest.as_deref(),
                    Some(frozen.baseline_digest())
                );
                BaselineScope::FrozenWorkflowBaseline
            } else {
                assert!(e.binding.baseline_digest.is_none());
                BaselineScope::Absent
            };
            source.insert(
                scope.clone(),
                ScopedSource {
                    source_snapshot_digest: e.binding.source_snapshot_digest.clone(),
                    baseline,
                },
            );
            names.insert(name, scope.clone());
            policies.insert(
                scope,
                EligibilityPolicy {
                    binding: e.binding,
                    producer: e.producer,
                    required_scopes: e.required_scopes,
                    contract_digest: e.contract_digest,
                    action: "commit".into(),
                    producer_principals: BTreeSet::from(["fixture-ci".into()]),
                    approval_principals: if name == "specguard" { BTreeMap::from([("review".into(),BTreeSet::from(["fixture-reviewer".into()]))]) } else { BTreeMap::new() },
                },
            );
        }
        let sources = ScopedSources::freeze_contexts(&b, &frozen, source).unwrap();
        write(out.join("frozen.json"), &frozen);
        write(out.join("sources.json"), &sources);
        write(out.join("policies.json"), &policies);
        write(out.join("scopes.json"), &names);
        flowguard::gate::prepare_scoped_gate(
            &b,
            &frozen,
            &sources,
            policies,
            flowguard::gate::GateRequest {
                run_id: "before-producer-results".into(),
                action: "commit".into(),
                started_at: "2026-10-09T00:00:00Z".into(),
            },
        )
        .unwrap();
    } else if mode == "finish" {
        let ac: CandidateSnapshot = read(out.join("archguard/candidate.json"));
        let p: GuardContract = read(out.join("archguard/policy.json"));
        let prepared =
            GitCargoEvidence::prepare(&repo, &ac, ProtectedCargoPolicy::freeze(p, vec![]).unwrap())
                .unwrap();
        let e: Expected = read(out.join("archguard/expected.json"));
        assert_eq!(prepared.binding(), &e.binding);
        let bundle = prepared.run(&AtomicBool::new(false)).unwrap();
        bundle.verify(&repo, &ac).unwrap();
        write(out.join("archguard/bundle.json"), &bundle);
        let c = bundle.cargo();
        write(out.join("archguard/envelope.json"), &c.envelope);
        for (name, value) in [
            ("contract", &c.contract),
            ("facts", &c.facts),
            ("report", &c.report),
        ] {
            write(
                out.join(format!("archguard/{name}.json")),
                value.as_ref().unwrap(),
            );
        }
        write(out.join("archguard/domain.json"), &c.domain);
        let mut q: CheckRequest = read(out.join("gitguard/request.json"));
        q.repo_root = root.into();
        let b = BoundCheck::prepare(q)
            .unwrap()
            .run(&AtomicBool::new(false))
            .unwrap();
        write(out.join("gitguard/bundle.json"), &b);
        write(out.join("gitguard/envelope.json"), &b.envelope);
        for (name, value) in [
            ("contract", &b.contract),
            ("facts", &b.facts),
            ("report", &b.report),
        ] {
            write(
                out.join(format!("gitguard/{name}.json")),
                value.as_ref().unwrap(),
            );
        }
        write(out.join("gitguard/domain.json"), &b.domain);
    } else {
        panic!("unknown generator mode")
    }
}
