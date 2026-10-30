mod common;
use common::*;
use gitguard::{
    candidate::CandidateRequest,
    cli::CheckRequest,
    evidence::{envelope::BoundCheck, projection::FrozenPolicy},
    execution::{self, ExecutionConfig, Operation},
    scope::TaskScope,
};
use guardengine::{Decision, Enforcement};
use std::sync::atomic::AtomicBool;
#[test]
fn every_writer_stays_denied_without_reviewed_external_authorization() {
    for enabled in [false, true] {
        for op in [
            Operation::CreateWorktree,
            Operation::CreateBranch,
            Operation::Push,
            Operation::Merge,
        ] {
            assert!(execution::request(&ExecutionConfig { enabled }, op).is_err());
        }
    }
    for extra in ["approved", "flowGate", "decision", "authorizationReviewed"] {
        let value = serde_json::json!({"enabled":true,extra:true});
        assert!(serde_json::from_value::<ExecutionConfig>(value).is_err());
    }
}
#[test]
fn real_read_only_allow_never_enables_writer_or_modifies_source() {
    let r = Repo::new("sha1");
    let before = r.state();
    let policy = FrozenPolicy::new(Enforcement::Enforce);
    let req = CheckRequest {
        api_version: "gitguard.check/v1alpha1".into(),
        repo_root: r.path().into(),
        repo_id: "repo".into(),
        candidate_oid: r.base.clone(),
        scope: TaskScope::advisory(
            "task",
            vec!["R".into()],
            vec![b"a".to_vec()],
            &policy.digest(),
            None,
        )
        .unwrap(),
        candidate: CandidateRequest {
            worktree_id: "w".into(),
            base_oid: r.base.clone(),
            merge_group_id: None,
            members: vec![],
        },
        contract: policy.contract(),
    };
    let result = BoundCheck::prepare(req)
        .unwrap()
        .run(&AtomicBool::new(false))
        .unwrap();
    assert_eq!(result.envelope.decision, Some(Decision::Allow));
    assert!(execution::request(&ExecutionConfig::default(), Operation::Merge).is_err());
    assert_eq!(before, r.state());
}
