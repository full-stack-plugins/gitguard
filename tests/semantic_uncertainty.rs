mod common;
use common::*;
use gitguard::{
    Repository,
    candidate::CandidateRequest,
    conflicts::{Dependency, DependencyKind, GraphCoverage, analyze_impacts},
    scope::TaskScope,
    subject::SubjectRequest,
};
#[test]
fn disjoint_files_shared_api_and_missing_required_index_are_not_safe() {
    let r = Repo::new("sha1");
    let head = r.commit("different", "candidate");
    let repo = Repository::discover(r.path(), "repo").unwrap();
    let subject = repo.resolve_subject(SubjectRequest::Commit(head)).unwrap();
    let req = CandidateRequest {
        worktree_id: "w".into(),
        base_oid: r.base.clone(),
        merge_group_id: None,
        members: vec![],
    };
    let snap = |task: &str| {
        repo.prepare_candidate(
            &subject,
            &TaskScope::advisory(
                task,
                vec![task.into()],
                vec![b"different".to_vec()],
                &"a".repeat(64),
                None,
            )
            .unwrap(),
            &req,
        )
        .unwrap()
    };
    let snapshots = vec![snap("provider"), snap("consumer")];
    let graph = GraphCoverage {
        version: "gitguard.graph-fixture/v1alpha1".into(),
        required: vec!["api".into(), "schema".into()],
        covered: vec!["api".into()],
        dependencies: vec![Dependency {
            provider_task: "provider".into(),
            consumer_task: "consumer".into(),
            symbol: "SharedApi".into(),
            kind: DependencyKind::Api,
        }],
    };
    let result = analyze_impacts(&snapshots, Some(&graph));
    assert_eq!(result.risks, vec!["SharedApi"]);
    assert_eq!(result.missing, vec!["schema"]);
    assert!(!result.complete);
    let absent = analyze_impacts(&snapshots, None);
    assert!(!absent.complete);
    assert!(absent.unknown);
}
