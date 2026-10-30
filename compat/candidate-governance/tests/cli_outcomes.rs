mod common;
use common::*;
use gitguard::{
    candidate::CandidateRequest,
    cli::CheckRequest,
    evidence::{envelope::BoundCheck, projection::FrozenPolicy},
    scope::TaskScope,
};
use guardengine::{
    Decision, Enforcement,
    integration::{EvidenceProfile, RunStatus},
};
use std::{
    io::Write,
    process::{Command, Stdio},
    sync::atomic::AtomicBool,
};
fn request(r: &Repo, oid: &str, enforcement: Enforcement, path: &str) -> CheckRequest {
    let policy = FrozenPolicy::new(enforcement);
    CheckRequest {
        api_version: "gitguard.check/v1alpha1".into(),
        repo_root: r.path().into(),
        repo_id: "repo".into(),
        candidate_oid: oid.into(),
        scope: TaskScope::advisory(
            "task",
            vec!["R".into()],
            vec![path.as_bytes().to_vec()],
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
    }
}
fn cli(value: &serde_json::Value, args: &[&str]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_gitguard"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(value).unwrap())
        .unwrap();
    child.wait_with_output().unwrap()
}
#[test]
fn cli_real_engine_bundle_exit_codes_and_report_match() {
    let r = Repo::new("sha1");
    let head = r.commit("a", "candidate");
    for (enforcement, path, code, decision) in [
        (Enforcement::Enforce, "a", 0, "ALLOW"),
        (Enforcement::Enforce, "other", 2, "BLOCK"),
        (Enforcement::Review, "other", 3, "REQUIRE_APPROVAL"),
        (Enforcement::Advise, "other", 0, "ALLOW"),
    ] {
        let req = request(&r, &head, enforcement, path);
        let out = cli(&serde_json::to_value(req).unwrap(), &["check"]);
        assert_eq!(
            out.status.code(),
            Some(code),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let bundle: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(bundle["envelope"]["decision"], decision);
        assert_eq!(bundle["report"]["decision"], decision);
        let env: guardengine::integration::GuardRunEnvelope =
            serde_json::from_value(bundle["envelope"].clone()).unwrap();
        guardengine::integration::verify_engine_artifacts(
            &env,
            &serde_json::to_vec(&bundle["contract"]).unwrap(),
            &serde_json::to_vec(&bundle["facts"]).unwrap(),
            &serde_json::to_vec(&bundle["report"]).unwrap(),
        )
        .unwrap();
    }
}
#[test]
fn invalid_input_has_no_envelope_and_report_flag_does_not_reuse_old_file() {
    let r = Repo::new("sha1");
    let mut value = serde_json::to_value(request(&r, &r.base, Enforcement::Enforce, "a")).unwrap();
    let mut unsupported = value.clone();
    unsupported["api_version"] = serde_json::json!("gitguard.check/v9");
    let unsupported = cli(&unsupported, &["check"]);
    assert_eq!(unsupported.status.code(), Some(4));
    assert!(unsupported.stdout.is_empty());
    value["candidate_oid"] = serde_json::json!("0".repeat(40));
    let out = cli(&value, &["check"]);
    assert_eq!(out.status.code(), Some(4));
    assert!(out.stdout.is_empty());
    value["approved"] = serde_json::json!(true);
    assert!(cli(&value, &["check"]).stdout.is_empty());
    let old = r.path().join("old.json");
    std::fs::write(&old, "old ALLOW").unwrap();
    let out = cli(&value, &["check", "--report", old.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(4));
    assert!(out.stdout.is_empty());
    assert_eq!(std::fs::read_to_string(old).unwrap(), "old ALLOW");
}
#[test]
fn bound_cancel_and_source_error_emit_null_decision() {
    let r = Repo::new("sha1");
    let bound = BoundCheck::prepare(request(&r, &r.base, Enforcement::Enforce, "a")).unwrap();
    let cancelled = bound.run(&AtomicBool::new(true)).unwrap();
    assert_eq!(cancelled.envelope.run_status, RunStatus::Cancelled);
    assert_eq!(cancelled.envelope.decision, None);
    cancelled
        .envelope
        .validate(EvidenceProfile::EngineBacked)
        .unwrap();
    let bound = BoundCheck::prepare(request(&r, &r.base, Enforcement::Enforce, "a")).unwrap();
    std::fs::remove_file(r.path().join(".git/index")).unwrap();
    let failed = bound.run(&AtomicBool::new(false)).unwrap();
    assert_eq!(failed.envelope.run_status, RunStatus::Error);
    assert_eq!(failed.envelope.decision, None);
    assert_eq!(
        failed.envelope.coverage.status,
        guardengine::integration::CoverageStatus::Partial
    );
    assert!(failed.envelope.coverage.observed_scopes.is_empty());
    assert_eq!(
        failed.envelope.coverage.required_scopes,
        failed.envelope.coverage.missing_scopes
    );
}
#[test]
fn dirty_partial_cli_is_block_indeterminate() {
    let r = Repo::new("sha1");
    std::fs::write(r.path().join("a"), "dirty").unwrap();
    let req = request(&r, &r.base, Enforcement::Review, "a");
    let out = cli(&serde_json::to_value(req).unwrap(), &["check"]);
    assert_eq!(out.status.code(), Some(2));
    let bundle: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        bundle["envelope"]["decision"],
        serde_json::to_value(Decision::Block).unwrap()
    );
    assert_eq!(
        bundle["report"]["evaluations"][0]["status"],
        "INDETERMINATE"
    );
}
