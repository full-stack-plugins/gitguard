mod common;
use gitguard::{
    candidate::CandidateRequest,
    cli::CheckRequest,
    evidence::{envelope::BoundCheck, projection::FrozenPolicy},
    scope::TaskScope,
};
use guardengine::{
    Enforcement,
    integration::{CoverageStatus, RunStatus},
};
use std::sync::atomic::AtomicBool;
#[test]
fn bounded_producer_refusal_retains_bound_error_without_report_or_decision() {
    let source = common::Repo::new("sha1");
    for i in 0..700 {
        std::fs::write(source.path().join(format!("path-{i}")), "same blob").unwrap();
    }
    let head = source.commit("a", "candidate");
    let before = source.state();
    let mut contract = FrozenPolicy::new(Enforcement::Enforce).contract();
    let rule = contract.spec.rules[0].clone();
    contract.spec.rules = (0..128)
        .map(|i| {
            let mut r = rule.clone();
            r.id = format!("rule-{i}");
            r
        })
        .collect();
    let policy = FrozenPolicy::from_contract(contract).unwrap();
    let bundle = BoundCheck::prepare(CheckRequest {
        api_version: "gitguard.check/v1alpha1".into(),
        repo_root: source.path().into(),
        repo_id: "repo".into(),
        candidate_oid: head,
        scope: TaskScope::advisory(
            "task",
            vec!["R".into()],
            vec![b"allowed".to_vec()],
            &policy.digest(),
            None,
        )
        .unwrap(),
        candidate: CandidateRequest {
            worktree_id: "w".into(),
            base_oid: source.base.clone(),
            merge_group_id: None,
            members: vec![],
        },
        contract: policy.contract(),
    })
    .unwrap()
    .run(&AtomicBool::new(false))
    .unwrap();
    assert_eq!(bundle.envelope.run_status, RunStatus::Error);
    assert!(bundle.envelope.decision.is_none());
    assert!(bundle.report.is_none());
    assert!(bundle.facts.is_none());
    assert!(bundle.contract.is_none());
    assert_eq!(bundle.envelope.coverage.status, CoverageStatus::Partial);
    assert!(bundle.envelope.coverage.observed_scopes.is_empty());
    assert_eq!(bundle.exit_code(), 4);
    assert_eq!(before, source.state());
}
