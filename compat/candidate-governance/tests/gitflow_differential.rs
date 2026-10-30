mod common;
use gitguard::{
    Repository, candidate::CandidateRequest, gitflow::*, scope::TaskScope, subject::SubjectRequest,
};
use serde_json::Value;
fn captured(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/adapters/gitflow/fixtures/{name}.stdout",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}
fn prepared(
    target: &str,
) -> (
    tempfile::TempDir,
    gitguard::candidate::CandidateSnapshot,
    PreparedCheck,
) {
    let dir = tempfile::tempdir().unwrap();
    common::git(
        dir.path(),
        &[
            "clone",
            "-q",
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/adapters/gitflow/fixtures/source.bundle"
            ),
            ".",
        ],
    );
    let meta: Value =
        serde_json::from_str(include_str!("../adapters/gitflow/fixtures/capture.json")).unwrap();
    let repo = Repository::discover(dir.path(), "fixture-repo").unwrap();
    let candidate = repo
        .prepare_candidate(
            &repo
                .resolve_subject(SubjectRequest::Commit(
                    meta["candidate_oid"].as_str().unwrap().into(),
                ))
                .unwrap(),
            &TaskScope::advisory(
                "task",
                vec!["R".into()],
                vec![b"a".to_vec()],
                &"a".repeat(64),
                None,
            )
            .unwrap(),
            &CandidateRequest {
                worktree_id: "w".into(),
                base_oid: meta["base_oid"].as_str().unwrap().into(),
                merge_group_id: None,
                members: vec![],
            },
        )
        .unwrap();
    let raw = include_bytes!("../adapters/gitflow/fixtures/workflow.json");
    let config = ExpectedWorkflow {
        profile: PROFILE.into(),
        source: "feature/task".into(),
        target: target.into(),
        policy_ref: candidate.base_oid().into(),
        raw_policy_digest: meta["policy_sha256"].as_str().unwrap().into(),
        native_policy_digest: "890bd93513538e41318f9a5d23e68036252c11f2ce2849aa84fd766217164378"
            .into(),
    };
    let p = PreparedCheck::freeze(&repo, &candidate, config, raw).unwrap();
    (dir, candidate, p)
}
#[test]
fn actual_native_statuses_are_not_gitguard_exit_codes_or_authority() {
    let (_d, c, p) = prepared("develop");
    assert_eq!(
        p.consume(
            &c,
            NativeOutput::Exited {
                code: 0,
                stdout: &captured("allow")
            }
        )
        .unwrap()
        .outcome(),
        Outcome::Compatible
    );
    assert_eq!(
        p.consume(
            &c,
            NativeOutput::Exited {
                code: 3,
                stdout: &captured("unknown-object")
            }
        )
        .unwrap()
        .outcome(),
        Outcome::Incomplete
    );
    assert!(matches!(
        p.consume(
            &c,
            NativeOutput::Exited {
                code: 2,
                stdout: &captured("usage")
            }
        ),
        Err(ImportError::NativeUsage)
    ));
    assert!(matches!(
        p.consume(
            &c,
            NativeOutput::Exited {
                code: 4,
                stdout: &captured("internal-error")
            }
        ),
        Err(ImportError::NativeInternal)
    ));
    let (_d, c, p) = prepared("main");
    assert_eq!(
        p.consume(
            &c,
            NativeOutput::Exited {
                code: 1,
                stdout: &captured("deny-target")
            }
        )
        .unwrap()
        .outcome(),
        Outcome::Denied
    );
}
#[test]
fn binding_and_exit_contradictions_fail_closed() {
    let (_d, c, p) = prepared("develop");
    for field in [
        "base",
        "head",
        "source",
        "target",
        "policy_ref",
        "policy_sha256",
        "schema_version",
        "action",
    ] {
        let mut v: Value = serde_json::from_slice(&captured("allow")).unwrap();
        v[field] = "wrong".into();
        assert!(
            p.consume(
                &c,
                NativeOutput::Exited {
                    code: 0,
                    stdout: &serde_json::to_vec(&v).unwrap()
                }
            )
            .is_err(),
            "{field}"
        );
    }
    for code in [1, 2, 3, 4, -1] {
        assert!(
            p.consume(
                &c,
                NativeOutput::Exited {
                    code,
                    stdout: &captured("allow")
                }
            )
            .is_err()
        );
    }
    let mut v: Value = serde_json::from_slice(&captured("allow")).unwrap();
    v["commits"][0]["decision"] = "deny".into();
    assert!(
        p.consume(
            &c,
            NativeOutput::Exited {
                code: 0,
                stdout: &serde_json::to_vec(&v).unwrap()
            }
        )
        .is_err()
    );
    let mut changed = serde_json::to_value(&c).unwrap();
    changed["requirement_ids"] = serde_json::json!(["OTHER"]);
    assert!(
        p.consume(
            &serde_json::from_value(changed).unwrap(),
            NativeOutput::Exited {
                code: 0,
                stdout: &captured("allow")
            }
        )
        .is_err()
    );
}
#[test]
fn consumer_faults_stay_distinct_from_native_unknown() {
    let (_d, c, p) = prepared("develop");
    assert!(matches!(
        p.consume(&c, NativeOutput::Timeout),
        Err(ImportError::Timeout)
    ));
    for raw in [
        b"{".to_vec(),
        b"{\"decision\":\"allow\",\"decision\":\"deny\"}".to_vec(),
        vec![b' '; 1024 * 1024 + 1],
    ] {
        assert!(
            p.consume(
                &c,
                NativeOutput::Exited {
                    code: 0,
                    stdout: &raw
                }
            )
            .is_err()
        );
    }
    let mut v: Value = serde_json::from_slice(&captured("allow")).unwrap();
    v.as_object_mut().unwrap().remove("policy_ref");
    assert!(
        p.consume(
            &c,
            NativeOutput::Exited {
                code: 0,
                stdout: &serde_json::to_vec(&v).unwrap()
            }
        )
        .is_err()
    );
}

#[test]
fn missing_native_rule_coverage_cannot_claim_compatible() {
    let (_d, c, p) = prepared("develop");
    for key in ["checks", "tag_checks"] {
        let mut v: Value = serde_json::from_slice(&captured("allow")).unwrap();
        if key == "checks" {
            v["commits"][0]["checks"] = serde_json::json!([]);
        } else {
            v[key] = serde_json::json!([]);
        }
        assert!(
            p.consume(
                &c,
                NativeOutput::Exited {
                    code: 0,
                    stdout: &serde_json::to_vec(&v).unwrap()
                }
            )
            .is_err(),
            "{key}"
        );
    }
}
#[test]
fn protected_configuration_and_raw_policy_are_frozen_independently() {
    let (d, c, p) = prepared("develop");
    let repo = Repository::discover(d.path(), "fixture-repo").unwrap();
    let meta: Value =
        serde_json::from_str(include_str!("../adapters/gitflow/fixtures/capture.json")).unwrap();
    for field in 0..6 {
        let mut config = ExpectedWorkflow {
            profile: PROFILE.into(),
            source: "feature/task".into(),
            target: "develop".into(),
            policy_ref: c.base_oid().into(),
            raw_policy_digest: meta["policy_sha256"].as_str().unwrap().into(),
            native_policy_digest:
                "890bd93513538e41318f9a5d23e68036252c11f2ce2849aa84fd766217164378".into(),
        };
        match field {
            0 => config.profile = "unknown".into(),
            1 => config.source = "../bad".into(),
            2 => config.target = "--apply".repeat(100),
            3 => config.policy_ref = "f".repeat(40),
            4 => config.raw_policy_digest = "f".repeat(64),
            _ => config.native_policy_digest = "f".repeat(64),
        }
        let frozen = PreparedCheck::freeze(
            &repo,
            &c,
            config,
            include_bytes!("../adapters/gitflow/fixtures/workflow.json"),
        );
        if field == 5 {
            let other = frozen.unwrap();
            assert_ne!(p.digest(), other.digest());
            assert!(
                other
                    .consume(
                        &c,
                        NativeOutput::Exited {
                            code: 0,
                            stdout: &captured("allow")
                        }
                    )
                    .is_err()
            );
        } else {
            assert!(frozen.is_err());
        }
    }
}
#[test]
fn retained_real_same_candidate_differential_keeps_both_policy_owners() {
    let meta: Value =
        serde_json::from_str(include_str!("../adapters/gitflow/fixtures/capture.json")).unwrap();
    assert_eq!(meta["source_bytes_unchanged"], true);
    assert_eq!(meta["gitguard"][0]["decision"], "ALLOW");
    assert_eq!(meta["gitguard"][1]["decision"], "BLOCK");
    for case in ["allow", "deny-target"] {
        let native: Value = serde_json::from_slice(&captured(case)).unwrap();
        assert_eq!(native["head"], meta["candidate_oid"]);
        assert_eq!(native["base"], meta["base_oid"]);
    }
    // Cartesian outcomes: agree allow, workflow-only deny, path-only deny, agree deny.
    assert_eq!(meta["native"][0]["exit"], 0);
    assert_eq!(meta["native"][1]["exit"], 1);
    assert_eq!(meta["native"][2]["exit"], 3);
    assert_eq!(meta["gitguard"][2]["exit"], 4);
}
