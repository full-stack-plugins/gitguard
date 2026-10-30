#[path = "../examples/joint_consumption.rs"]
mod example;
#[test]
#[cfg(feature = "privileged-execution")]
fn four_real_candidate_groups_consume_without_implicit_write_authority() {
    let report = example::run().unwrap();
    assert_eq!(report["groups"].as_object().unwrap().len(), 4);
    assert_eq!(report["original_artifacts_unchanged"], true);
    assert_eq!(report["m1_late_publication_refused"], true);
    assert_eq!(report["execution"]["cancelled_ref_unchanged"], true);
}
#[test]
#[cfg(not(feature = "privileged-execution"))]
fn default_build_does_not_run_the_local_mutation_fixture() {
    assert!(example::run().unwrap_err().contains("privileged-execution"));
}
