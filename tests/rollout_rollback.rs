#[allow(dead_code)]
#[path = "../examples/local_rollout.rs"]
mod rollout;
#[test]
fn local_rollout_preserves_history_and_stops_unresolved_or_disabled_writes() {
    if !cfg!(feature = "privileged-execution") {
        assert!(rollout::run().is_err());
        return;
    }
    let report = rollout::run().unwrap();
    assert_eq!(report["source_unchanged"], true);
    assert_eq!(report["advisory"]["write"], "Disabled");
    assert_eq!(report["shadow"]["comparison"], "agree");
    assert_eq!(report["unresolved_difference"]["write"], "Disabled");
    assert_eq!(report["unresolved_difference"]["enabled"], false);
    assert_eq!(report["opt_in"]["observed_target"], report["candidate_oid"]);
    assert_eq!(report["adapter_rollback"]["scheduled_write"], false);
    assert_eq!(report["executor_rollback"]["write"], "Disabled");
    assert_eq!(report["executor_rollback"]["shadow_still_available"], true);
    assert_eq!(report["history_unchanged_after_rollback"], true);
    assert_eq!(report["ack_after_rollback"], "Applied");
    assert_eq!(report["unknown_after_rollback"], "RecoveryRequired");
    assert_eq!(report["production_authority"], false);
}

#[test]
fn required_specialists_guard_actual_m2_rollout_dispatch() {
    if !cfg!(feature = "privileged-execution") {
        return;
    }
    let required = rollout::required_specialist_rollout().unwrap();
    assert_eq!(required["required_count"], 5);
    assert_eq!(required["rejected_before_grant"], 5);
    assert_eq!(required["candidate"], required["actual_target"]);
    assert_eq!(required["adapter_rollback_required_set_preserved"], true);
    assert_eq!(
        required["executor_rollback"],
        "Disabled; read-only consumption still eligible"
    );
    assert_eq!(required["history_unchanged"], true);
    let gitflow: serde_json::Value =
        serde_json::from_str(include_str!("../adapters/gitflow/fixtures/capture.json")).unwrap();
    assert_ne!(required["candidate"], gitflow["candidate_oid"]);
    assert_eq!(required["technical_specguard"], "REQUIRE_APPROVAL");
    println!("{}", serde_json::to_string_pretty(&required).unwrap());
}
