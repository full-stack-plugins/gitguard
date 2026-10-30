mod common;
use common::*;
use gitguard::{
    Repository,
    candidate::CandidateRequest,
    evidence::projection::{FrozenPolicy, project},
    preflight::preflight,
    scope::TaskScope,
    subject::SubjectRequest,
};
use guardengine::{Completeness, Decision, Enforcement, RuleStatus};
#[test]
fn actual_git_scope_violation_uses_real_engine_enforce_review_advise() {
    let r = Repo::new("sha1");
    let head = r.commit("a", "candidate");
    let repo = Repository::discover(r.path(), "repo").unwrap();
    for (enforcement, expected) in [
        (Enforcement::Enforce, Decision::Block),
        (Enforcement::Review, Decision::RequireApproval),
        (Enforcement::Advise, Decision::Allow),
    ] {
        let policy = FrozenPolicy::new(enforcement);
        let scope = TaskScope::advisory(
            "task",
            vec!["R".into()],
            vec![b"other".to_vec()],
            &policy.digest(),
            None,
        )
        .unwrap();
        let request = CandidateRequest {
            worktree_id: "w".into(),
            base_oid: r.base.clone(),
            merge_group_id: None,
            members: vec![],
        };
        let result = preflight(
            &repo,
            SubjectRequest::Commit(head.clone()),
            &scope,
            &request,
        )
        .unwrap();
        let facts = project(&repo, &result, &policy).unwrap();
        assert_eq!(facts.completeness, Completeness::Complete);
        assert_eq!(facts.facts.len(), 1);
        let report = guardengine::evaluate(&policy.contract(), &facts).unwrap();
        assert_eq!(report.decision, expected);
        let mut extended = serde_json::to_value(&report).unwrap();
        extended["candidateOid"] = serde_json::json!(head);
        assert!(serde_json::from_value::<guardengine::GuardReport>(extended).is_err());
        let mut extended = serde_json::to_value(&facts).unwrap();
        extended["grant"] = serde_json::json!({"approved":true});
        assert!(serde_json::from_value::<guardengine::GuardFacts>(extended).is_err());
        assert!(guardengine::verify_report(&report, &policy.contract(), &facts).unwrap());
    }
}
#[test]
fn dirty_commit_is_partial_block_and_policy_digest_cannot_be_asserted() {
    let r = Repo::new("sha1");
    std::fs::write(r.path().join("a"), "dirty").unwrap();
    let repo = Repository::discover(r.path(), "repo").unwrap();
    let policy = FrozenPolicy::new(Enforcement::Review);
    let request = CandidateRequest {
        worktree_id: "w".into(),
        base_oid: r.base.clone(),
        merge_group_id: None,
        members: vec![],
    };
    let scope = TaskScope::advisory(
        "task",
        vec!["R".into()],
        vec![b"a".to_vec()],
        &policy.digest(),
        None,
    )
    .unwrap();
    let result = preflight(
        &repo,
        SubjectRequest::Commit(r.base.clone()),
        &scope,
        &request,
    )
    .unwrap();
    let facts = project(&repo, &result, &policy).unwrap();
    assert_eq!(facts.completeness, Completeness::Partial);
    let report = guardengine::evaluate(&policy.contract(), &facts).unwrap();
    assert_eq!(report.decision, Decision::Block);
    assert_eq!(report.evaluations[0].status, RuleStatus::Indeterminate);
    let wrong = TaskScope::advisory(
        "task",
        vec!["R".into()],
        vec![b"a".to_vec()],
        &"b".repeat(64),
        None,
    )
    .unwrap();
    let wrong = preflight(
        &repo,
        SubjectRequest::Commit(r.base.clone()),
        &wrong,
        &request,
    )
    .unwrap();
    assert!(project(&repo, &wrong, &policy).is_err());
}
#[test]
fn bounded_strict_mapping_rejects_unsupported_contracts_and_fields() {
    let policy = FrozenPolicy::new(Enforcement::Enforce);
    let mut wrong = policy.contract();
    wrong.api_version = "guard.partme.ai/v9".into();
    assert!(FrozenPolicy::from_contract(wrong).is_err());
    let mut unknown = serde_json::to_value(policy.contract()).unwrap();
    unknown["candidateOid"] = serde_json::json!("0".repeat(40));
    assert!(serde_json::from_value::<guardengine::GuardContract>(unknown).is_err());
    let mut wrong = policy.contract();
    wrong.spec.rules[0].assertion = guardengine::GuardAssertion::ForbidRelation {
        subject: "git-candidate".into(),
        predicate: "allows".into(),
        object: "task-scope".into(),
    };
    assert!(FrozenPolicy::from_contract(wrong).is_err());
    let mut huge = policy.contract();
    let rule = huge.spec.rules[0].clone();
    huge.spec.rules = (0..129)
        .map(|i| {
            let mut r = rule.clone();
            r.id = format!("rule-{i}");
            r
        })
        .collect();
    assert!(FrozenPolicy::from_contract(huge).is_err());
}
