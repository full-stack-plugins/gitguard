mod common;
use common::git;
use gitguard::{
    Repository,
    candidate::{CandidateRequest, CandidateSnapshot},
    codegraph::{self, Profile},
    scope::TaskScope,
    subject::SubjectRequest,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
const RAW: &[u8] = include_bytes!("fixtures/codegraph-1072/query.json");
const HEAD: &str = "ebadafa7d34a3b8974d6df1beb271deded7c731a";
fn digest(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}
fn fixture() -> (
    tempfile::TempDir,
    Repository,
    Vec<CandidateSnapshot>,
    Profile,
) {
    let d = tempfile::tempdir().unwrap();
    git(d.path(), &["init", "-q"]);
    let bundle = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/codegraph-1072/source.bundle");
    git(d.path(), &["fetch", bundle.to_str().unwrap(), "HEAD"]);
    git(d.path(), &["checkout", "-q", "FETCH_HEAD"]);
    let repo = Repository::discover(d.path(), "repo").unwrap();
    let subject = repo
        .resolve_subject(SubjectRequest::Commit(HEAD.into()))
        .unwrap();
    let file_tasks: BTreeMap<String, String> = BTreeMap::from([
        ("caller.js".into(), "task-A".into()),
        ("callee.js".into(), "task-B".into()),
    ]);
    let mut snapshots = vec![];
    for (file, task) in &file_tasks {
        let scope = TaskScope::advisory(
            task,
            vec!["R".into()],
            vec![file.as_bytes().to_vec()],
            &"a".repeat(64),
            None,
        )
        .unwrap();
        snapshots.push(
            repo.prepare_candidate(
                &subject,
                &scope,
                &CandidateRequest {
                    worktree_id: task.clone(),
                    base_oid: HEAD.into(),
                    merge_group_id: None,
                    members: vec![],
                },
            )
            .unwrap(),
        );
    }
    let p = Profile {
        source_revision: codegraph::SOURCE_REVISION.into(),
        export_sha256: digest(RAW),
        candidate_oid: HEAD.into(),
        file_tasks,
    };
    (d, repo, snapshots, p)
}
#[test]
fn actual_codegraph_static_call_is_risk_not_semantic_completeness() {
    let (_d, repo, s, p) = fixture();
    let got = codegraph::analyze(&repo, &s, RAW, &p).unwrap();
    assert_eq!(got.risks, vec!["task-A consumes task-B: caller -> callee"]);
    assert!(got.unknown);
    assert!(!got.complete);
    assert_eq!(got.missing, vec!["semantic-completeness"]);
}

#[test]
fn changed_source_versions_coverage_and_controller_mapping_fail_closed() {
    let (_d, repo, s, mut p) = fixture();
    assert!(codegraph::analyze(&repo, &s, RAW, &p).is_ok());
    p.source_revision = "unknown-tool".into();
    assert!(codegraph::analyze(&repo, &s, RAW, &p).is_err());
    p.source_revision = codegraph::SOURCE_REVISION.into();
    p.export_sha256 = "0".repeat(64);
    assert!(codegraph::analyze(&repo, &s, RAW, &p).is_err());
    p.export_sha256 = digest(RAW);
    p.file_tasks.remove("callee.js");
    assert!(codegraph::analyze(&repo, &s, RAW, &p).is_err());
    p.file_tasks
        .insert("callee.js".into(), "unknown-task".into());
    assert!(codegraph::analyze(&repo, &s, RAW, &p).is_err());
    p.file_tasks.insert("callee.js".into(), "task-B".into());
    let original: serde_json::Value = serde_json::from_slice(RAW).unwrap();
    for (pointer, replacement) in [
        ("/tables/schema_versions/1/version", serde_json::json!(12)),
        (
            "/tables/project_metadata/0/value",
            serde_json::json!("0".repeat(40)),
        ),
        (
            "/tables/project_metadata/1/value",
            serde_json::json!(format!(
                "{{\"commit\":\"{HEAD}\",\"paths\":[\"caller.js\"]}}"
            )),
        ),
        (
            "/tables/project_metadata/3/value",
            serde_json::json!("1.7.0"),
        ),
        ("/tables/project_metadata/4/value", serde_json::json!("28")),
        (
            "/tables/files/0/content_hash",
            serde_json::json!("0".repeat(64)),
        ),
        ("/tables/files/0/errors", serde_json::json!("parse failure")),
        ("/tables/files/0/language", serde_json::json!("unknown")),
        (
            "/tables/unresolved_refs",
            serde_json::json!([{"status":"failed"}]),
        ),
        (
            "/tables/nodes/0/id",
            serde_json::json!("missing-edge-endpoint"),
        ),
    ] {
        let mut changed = original.clone();
        *changed.pointer_mut(pointer).unwrap() = replacement;
        let raw = serde_json::to_vec(&changed).unwrap();
        // Re-freeze transport bytes so this checks interpretation, not just hash mismatch.
        p.export_sha256 = digest(&raw);
        assert!(
            codegraph::analyze(&repo, &s, &raw, &p).is_err(),
            "{pointer}"
        );
    }
}
#[test]
fn empty_edges_do_not_become_complete_or_no_conflict_proof() {
    let (_d, repo, s, mut p) = fixture();
    let mut v: serde_json::Value = serde_json::from_slice(RAW).unwrap();
    v["tables"]["edges"] = serde_json::json!([]);
    let raw = serde_json::to_vec(&v).unwrap();
    p.export_sha256 = digest(&raw);
    let result = codegraph::analyze(&repo, &s, &raw, &p).unwrap();
    assert!(result.risks.is_empty() && result.unknown && !result.complete);
}
