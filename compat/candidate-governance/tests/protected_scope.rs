mod common;
use common::{Repo, git};
use gitguard::{
    Repository,
    candidate::CandidateRequest,
    preflight::preflight_protected,
    scope::{ImmutableScopeSource, ProtectedScopeRequest, ProtectedTaskScope},
    subject::SubjectRequest,
};
use sha2::{Digest, Sha256};
fn digest(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}
const POLICY: &str = r#"{"schema_version":"gitguard.scope-policy/v1alpha1","task_id":"task","requirement_ids":["R1"],"allowed_paths":[[97]]}"#;
fn request(oid: &str) -> ProtectedScopeRequest {
    ProtectedScopeRequest {
        repo_id: "repo".into(),
        task_id: "task".into(),
        requirement_ids: vec!["R1".into()],
        allowed_paths: vec![b"a".to_vec()],
        policy: ImmutableScopeSource {
            commit_oid: oid.into(),
            path: b"policy.json".to_vec(),
            digest: digest(POLICY.as_bytes()),
        },
        baseline: None,
    }
}
#[test]
fn immutable_policy_survives_candidate_deletion_and_remains_advisory() {
    for format in ["sha1", "sha256"] {
        let r = Repo::new(format);
        let base = r.commit("policy.json", POLICY);
        git(r.path(), &["rm", "policy.json"]);
        git(r.path(), &["commit", "-qm", "delete"]);
        let head = git(r.path(), &["rev-parse", "HEAD"]);
        let repo = Repository::discover(r.path(), "repo").unwrap();
        let before = r.state();
        let protected = ProtectedTaskScope::freeze(&repo, &request(&base)).unwrap();
        let result = preflight_protected(
            &repo,
            SubjectRequest::Commit(head),
            &protected,
            &CandidateRequest {
                worktree_id: "w".into(),
                base_oid: base,
                merge_group_id: None,
                members: vec![],
            },
        )
        .unwrap();
        assert_eq!(result.scope.requirement_ids(), ["R1"]);
        assert_eq!(result.changes.violations, [b"policy.json".to_vec()]);
        assert!(!result.complete);
        assert!(result.candidate.advisory());
        assert_eq!(result.candidate.baseline_digest(), None);
        assert_eq!(result.candidate.policy_digest(), protected.digest());
        assert_eq!(before, r.state());
    }
}
#[test]
fn rejects_unverified_source_and_markdown_approval() {
    let r = Repo::new("sha1");
    let base = r.commit("policy.json", POLICY);
    let md = r.commit("policy.json", "accepted");
    let repo = Repository::discover(r.path(), "repo").unwrap();
    let mut req = request(&base);
    req.policy.digest = "0".repeat(64);
    assert!(ProtectedTaskScope::freeze(&repo, &req).is_err());
    req = request(&md);
    req.policy.digest = digest(b"accepted");
    assert!(ProtectedTaskScope::freeze(&repo, &req).is_err());
    req = request(&base);
    req.requirement_ids = vec!["R2".into()];
    assert!(ProtectedTaskScope::freeze(&repo, &req).is_err());
    req = request(&base);
    req.task_id = "x".repeat(1024 * 1024);
    assert!(matches!(
        ProtectedTaskScope::freeze(&repo, &req),
        Err(gitguard::Diagnostic::LimitExceeded)
    ));
}

#[test]
fn baseline_and_policy_revision_changes_require_new_scope_digest() {
    let r = Repo::new("sha1");
    let policy = r.commit("policy.json", POLICY);
    let old = r.commit("baseline.md", "accepted old declaration");
    let newer = r.commit("baseline.md", "accepted new declaration");
    let repo = Repository::discover(r.path(), "repo").unwrap();
    let mut a = request(&policy);
    a.baseline = Some(ImmutableScopeSource {
        commit_oid: old.clone(),
        path: b"baseline.md".to_vec(),
        digest: digest(b"accepted old declaration"),
    });
    let frozen = ProtectedTaskScope::freeze(&repo, &a).unwrap();
    assert!(frozen.task_scope().validate().is_ok());
    let mut b = a.clone();
    b.baseline.as_mut().unwrap().commit_oid = newer.clone();
    assert!(ProtectedTaskScope::freeze(&repo, &b).is_err());
    b.baseline.as_mut().unwrap().digest = digest(b"accepted new declaration");
    let next = ProtectedTaskScope::freeze(&repo, &b).unwrap();
    assert_ne!(frozen.digest(), next.digest());
    b = a.clone();
    b.policy.commit_oid = newer;
    assert_ne!(
        frozen.digest(),
        ProtectedTaskScope::freeze(&repo, &b).unwrap().digest()
    );
    assert!(frozen.validate_sources(&repo).is_ok());
    let desc: serde_json::Value =
        serde_json::from_slice(&frozen.descriptor_json().unwrap()).unwrap();
    assert_eq!(desc["advisory"], true);
    assert_eq!(desc["trust_profile"], "local-controller-source-only");
    assert_eq!(desc["baseline"]["reference"]["commit_oid"], old);
    // Identical bytes and caller repo label still cannot substitute a different root.
    let other = Repo::new("sha1");
    other.commit("policy.json", POLICY);
    other.commit("baseline.md", "accepted old declaration");
    let foreign = Repository::discover(other.path(), "repo").unwrap();
    assert!(frozen.validate_sources(&foreign).is_err());
}
#[test]
fn strict_committed_policy_rejects_weakening_ambiguity_and_noncommit_refs() {
    let r = Repo::new("sha1");
    let base = r.commit("policy.json", POLICY);
    for body in [
        POLICY.replace("R1", "R2"),
        POLICY.replace(
            "\"task_id\":\"task\"",
            "\"task_id\":\"task\",\"task_id\":\"task\"",
        ),
        POLICY.replace(
            "\"task_id\":\"task\"",
            "\"task_id\":\"task\",\"approved\":true",
        ),
        POLICY.replace("v1alpha1", "v999"),
        " ".repeat(65537),
    ] {
        let oid = r.commit("policy.json", &body);
        let repo = Repository::discover(r.path(), "repo").unwrap();
        let mut req = request(&oid);
        req.policy.digest = digest(body.as_bytes());
        assert!(ProtectedTaskScope::freeze(&repo, &req).is_err());
    }
    let repo = Repository::discover(r.path(), "repo").unwrap();
    for oid in [
        "HEAD".into(),
        "0".repeat(40),
        git(r.path(), &["rev-parse", &format!("{base}:policy.json")]),
        git(r.path(), &["rev-parse", &format!("{base}^{{tree}}")]),
    ] {
        let mut req = request(&base);
        req.policy.commit_oid = oid;
        assert!(ProtectedTaskScope::freeze(&repo, &req).is_err());
    }
    let mut req = request(&base);
    req.policy.path = b"missing".to_vec();
    assert!(ProtectedTaskScope::freeze(&repo, &req).is_err());
    req.policy.path = b"../policy.json".to_vec();
    assert!(ProtectedTaskScope::freeze(&repo, &req).is_err());
}
#[test]
fn metadata_and_candidate_request_are_bounded_before_use() {
    let r = Repo::new("sha1");
    let base = r.commit("policy.json", POLICY);
    let repo = Repository::discover(r.path(), "repo").unwrap();
    for variant in 0..5 {
        let mut req = request(&base);
        match variant {
            0 => req.allowed_paths = vec![vec![b'a'; 4096]; 256],
            1 => req.requirement_ids = vec!["R".into(); 257],
            2 => req.policy.path = vec![b'a'; 4097],
            3 => req.policy.commit_oid = "a".repeat(65),
            _ => req.allowed_paths = vec![b"a".to_vec(); 257],
        };
        assert!(matches!(
            ProtectedTaskScope::freeze(&repo, &req),
            Err(gitguard::Diagnostic::LimitExceeded)
        ));
    }
    let frozen = ProtectedTaskScope::freeze(&repo, &request(&base)).unwrap();
    let req = CandidateRequest {
        worktree_id: "w".repeat(1024 * 1024),
        base_oid: base.clone(),
        merge_group_id: None,
        members: vec![],
    };
    assert!(matches!(
        preflight_protected(&repo, SubjectRequest::Commit(base), &frozen, &req),
        Err(gitguard::Diagnostic::LimitExceeded)
    ));
}

#[test]
fn candidate_weakening_cannot_replace_controller_policy() {
    let r = Repo::new("sha1");
    let base = r.commit("policy.json", POLICY);
    let head = r.commit("policy.json", &POLICY.replace("R1", "R2"));
    let repo = Repository::discover(r.path(), "repo").unwrap();
    let frozen = ProtectedTaskScope::freeze(&repo, &request(&base)).unwrap();
    let result = preflight_protected(
        &repo,
        SubjectRequest::Commit(head),
        &frozen,
        &CandidateRequest {
            worktree_id: "w".into(),
            base_oid: base,
            merge_group_id: None,
            members: vec![],
        },
    )
    .unwrap();
    assert_eq!(result.scope.requirement_ids(), ["R1"]);
    assert!(!result.complete);
    assert_eq!(result.changes.violations, [b"policy.json".to_vec()]);
}
