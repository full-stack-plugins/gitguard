//! Opt-in current-consumer execution over unmodified, provenance-pinned actual producer captures.
#[test]
#[ignore = "requires explicitly selected built GitGuard and FlowGuard binaries"]
fn retained_five_guard_gate_has_current_candidate_validation_and_no_write_authority() {
    use sha2::{Digest, Sha256};
    let gitguard = std::env::var("GITGUARD_BINARY").expect("explicit GitGuard binary");
    let flowguard = std::env::var("FLOWGUARD_BINARY").expect("explicit FlowGuard binary");
    let hash = |path: &str| format!("{:x}", Sha256::digest(std::fs::read(path).unwrap()));
    let output = tempfile::tempdir().unwrap();
    let capture = output.path().join("capture");
    let result = std::process::Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/fixtures/end-to-end/run.py"
        ))
        .args([
            "--gitguard",
            &gitguard,
            "--gitguard-sha256",
            &hash(&gitguard),
            "--flowguard",
            &flowguard,
            "--flowguard-sha256",
            &hash(&flowguard),
        ])
        .arg("--output")
        .arg(&capture)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["profile"], "gitguard.local-five-producer-replay/v1");
    assert_eq!(report["production_authenticated"], false);
    assert_eq!(report["execution_authorized"], false);
    assert_eq!(report["source_unchanged"], true);
    assert_eq!(report["cases"]["allow"]["exit"], 0);
    for case in ["missing_approval", "expired_approval", "revoked_approval"] {
        assert_eq!(report["cases"][case]["exit"], 4, "{case}");
        assert!(report["cases"][case]["decision"].is_null());
    }
    assert_eq!(report["cases"]["missing_specialist"]["exit"], 2);
    assert_eq!(report["cases"]["cancelled"]["exit"], 4);
    assert_eq!(report["cases"]["other_task"]["exit"], 4);
    assert_eq!(report["cases"]["other_candidate"]["exit"], 4);
    assert_eq!(report["retained_specguard_decision"], "REQUIRE_APPROVAL");
}
