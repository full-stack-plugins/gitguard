mod common;
use gitguard::{Repository, candidate::CandidateSnapshot, evidence::specialists::*};
use guardengine::integration::{eligibility::*, *};
use std::collections::{BTreeMap, BTreeSet};
const NOW: i64 = 2_000_000_000;
const PROVIDERS: [&str; 5] = [
    "archguard",
    "codeguard",
    "gitguard",
    "specguard",
    "testguard",
];
fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/fixtures/end-to-end/actual/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}
fn value(name: &str) -> serde_json::Value {
    serde_json::from_slice(&fixture(name)).unwrap()
}
struct Fixture {
    _dir: tempfile::TempDir,
    repo: Repository,
    candidate: CandidateSnapshot,
    required: Vec<RequiredSpecialist>,
    envelopes: BTreeMap<String, GuardRunEnvelope>,
    bytes: ImmutableArtifactBytes,
    authority: FixtureAuthority,
}
struct FixtureAuthority {
    records: BTreeMap<String, ProducerRecord>,
    approvals: BTreeMap<String, ApprovalRecord>,
}
impl AuthorityProvider for FixtureAuthority {
    fn verify_producer(
        &self,
        _: &GuardRunEnvelope,
        digest: &str,
    ) -> Result<ProducerRecord, AuthorityError> {
        self.records
            .get(digest)
            .cloned()
            .ok_or(AuthorityError::Untrusted)
    }
    fn verify_approval(&self, reference: &str) -> Result<ApprovalRecord, AuthorityError> {
        self.approvals
            .get(reference)
            .cloned()
            .ok_or(AuthorityError::Untrusted)
    }
}
fn valid() -> Validity {
    Validity {
        issued_at: NOW - 10,
        expires_at: NOW + 100,
        revoked: false,
    }
}
fn setup() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    common::git(
        dir.path(),
        &[
            "clone",
            "-q",
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/fixtures/end-to-end/actual/source.bundle"
            ),
            "repo",
        ],
    );
    let repo = Repository::discover(&dir.path().join("repo"), "cross-guard-repo").unwrap();
    let candidate: CandidateSnapshot =
        serde_json::from_slice(&fixture("gitguard/candidate.json")).unwrap();
    let policies = value("policies.json");
    let scopes = value("scopes.json");
    let mut required = vec![];
    let mut envelopes = BTreeMap::new();
    let mut artifacts = vec![];
    let mut authority = FixtureAuthority {
        records: BTreeMap::new(),
        approvals: BTreeMap::new(),
    };
    for name in PROVIDERS {
        let envelope: GuardRunEnvelope =
            serde_json::from_slice(&fixture(&format!("{name}/envelope.json"))).unwrap();
        let p = &policies[scopes[name].as_str().unwrap()];
        let policy = EligibilityPolicy {
            binding: serde_json::from_value(p["binding"].clone()).unwrap(),
            producer: serde_json::from_value(p["producer"].clone()).unwrap(),
            required_scopes: serde_json::from_value(p["required_scopes"].clone()).unwrap(),
            contract_digest: p["contract_digest"].as_str().unwrap().into(),
            action: "commit".into(),
            producer_principals: BTreeSet::from(["fixture-ci".into()]),
            approval_principals: serde_json::from_value(p["approval_principals"].clone()).unwrap(),
        };
        let baseline = policy.binding.baseline_digest.as_ref().map(|_| {
            let mut baseline_policy = policy.clone();
            baseline_policy.approval_principals = BTreeMap::from([(
                "specification-baseline".into(),
                BTreeSet::from(["fixture-baseline-owner".into()]),
            )]);
            let reference = format!("fixture:{name}-baseline");
            authority.approvals.insert(
                reference.clone(),
                ApprovalRecord {
                    principal: "fixture-baseline-owner".into(),
                    purpose: "specification-baseline".into(),
                    action: policy.action.clone(),
                    binding: policy.binding.clone(),
                    contract_digest: policy.contract_digest.clone(),
                    validity: valid(),
                },
            );
            BaselineApproval {
                reference,
                policy: baseline_policy,
                purpose: "specification-baseline".into(),
            }
        });
        for reference in &envelope.approval_refs {
            authority.approvals.insert(
                reference.clone(),
                ApprovalRecord {
                    principal: "fixture-reviewer".into(),
                    purpose: "review".into(),
                    action: policy.action.clone(),
                    binding: policy.binding.clone(),
                    contract_digest: policy.contract_digest.clone(),
                    validity: valid(),
                },
            );
        }
        let digest = guardengine::digest_json(&envelope).unwrap();
        authority.records.insert(
            digest.clone(),
            ProducerRecord {
                principal: "fixture-ci".into(),
                producer: envelope.producer.clone(),
                envelope_digest: digest,
                validity: valid(),
            },
        );
        for (key, reference) in [
            ("contract", envelope.artifacts.contract.as_ref()),
            ("facts", envelope.artifacts.facts.as_ref()),
            ("report", envelope.artifacts.report.as_ref()),
        ] {
            artifacts.push((
                reference.unwrap().clone(),
                fixture(&format!("{name}/{key}.json")),
            ));
        }
        required.push(RequiredSpecialist {
            id: name.into(),
            policy,
            profile: EvidenceProfile::EngineBacked,
            baseline,
        });
        envelopes.insert(name.into(), envelope);
    }
    Fixture {
        _dir: dir,
        repo,
        candidate,
        required,
        envelopes,
        bytes: ImmutableArtifactBytes::new(artifacts).unwrap(),
        authority,
    }
}
#[test]
fn actual_five_domains_preserve_review_and_recheck_baseline_and_issuer() {
    let mut f = setup();
    let original = serde_json::to_vec(&f.envelopes["specguard"]).unwrap();
    let prepared =
        PreparedSpecialistConsumption::freeze(&f.repo, &f.candidate, f.required).unwrap();
    let run = prepared
        .consume(
            &f.repo,
            &f.candidate,
            &f.envelopes,
            &f.bytes,
            &f.authority,
            NOW,
            None,
        )
        .unwrap();
    assert!(run.eligible());
    assert_eq!(
        run.results()["specguard"].technical_decision,
        Some(guardengine::Decision::RequireApproval)
    );
    assert_eq!(
        original,
        serde_json::to_vec(&f.envelopes["specguard"]).unwrap()
    );
    f.authority
        .approvals
        .get_mut("fixture:specguard-baseline")
        .unwrap()
        .validity
        .expires_at = NOW;
    assert!(
        !prepared
            .consume(
                &f.repo,
                &f.candidate,
                &f.envelopes,
                &f.bytes,
                &f.authority,
                NOW,
                None
            )
            .unwrap()
            .eligible()
    );
    f.authority
        .approvals
        .get_mut("fixture:specguard-baseline")
        .unwrap()
        .validity
        .expires_at = NOW + 100;
    f.authority
        .approvals
        .get_mut("fixture:specguard-baseline")
        .unwrap()
        .validity
        .revoked = true;
    assert!(
        !prepared
            .consume(
                &f.repo,
                &f.candidate,
                &f.envelopes,
                &f.bytes,
                &f.authority,
                NOW,
                None
            )
            .unwrap()
            .eligible()
    );
}

#[test]
fn missing_domain_wrong_candidate_source_and_old_baseline_cannot_cross_satisfy() {
    let mut f = setup();
    let prepared =
        PreparedSpecialistConsumption::freeze(&f.repo, &f.candidate, f.required).unwrap();
    let original = f.envelopes.remove("archguard").unwrap();
    assert!(
        prepared
            .consume(
                &f.repo,
                &f.candidate,
                &f.envelopes,
                &f.bytes,
                &f.authority,
                NOW,
                None
            )
            .is_err()
    );
    f.envelopes.insert("archguard".into(), original);
    for changed in ["candidate", "source", "baseline"] {
        let original = f.envelopes["specguard"].clone();
        let envelope = f.envelopes.get_mut("specguard").unwrap();
        match changed {
            "candidate" => envelope.binding.candidate_oid = envelope.binding.base_oid.clone(),
            "source" => {
                envelope.binding.source_snapshot_digest = format!("sha256:{}", "a".repeat(64))
            }
            _ => envelope.binding.baseline_digest = Some(format!("sha256:{}", "b".repeat(64))),
        }
        let run = prepared
            .consume(
                &f.repo,
                &f.candidate,
                &f.envelopes,
                &f.bytes,
                &f.authority,
                NOW,
                None,
            )
            .unwrap();
        assert!(!run.eligible(), "{changed}");
        assert_eq!(
            run.results()["specguard"].code,
            EligibilityCode::BindingChanged
        );
        f.envelopes.insert("specguard".into(), original);
    }
    let baseline = f
        .authority
        .approvals
        .get_mut("fixture:specguard-baseline")
        .unwrap();
    baseline.binding.baseline_digest = Some(format!("sha256:{}", "a".repeat(64)));
    assert!(
        !prepared
            .consume(
                &f.repo,
                &f.candidate,
                &f.envelopes,
                &f.bytes,
                &f.authority,
                NOW,
                None
            )
            .unwrap()
            .eligible()
    );
}

#[test]
fn fresh_authority_checks_reject_expiry_review_revocation_forgery_and_rehashing() {
    let mut f = setup();
    let prepared =
        PreparedSpecialistConsumption::freeze(&f.repo, &f.candidate, f.required).unwrap();
    assert!(
        !prepared
            .consume(
                &f.repo,
                &f.candidate,
                &f.envelopes,
                &f.bytes,
                &f.authority,
                NOW + 100,
                None
            )
            .unwrap()
            .eligible()
    );
    f.authority
        .approvals
        .get_mut("fixture:actual-spec-review")
        .unwrap()
        .validity
        .revoked = true;
    let run = prepared
        .consume(
            &f.repo,
            &f.candidate,
            &f.envelopes,
            &f.bytes,
            &f.authority,
            NOW,
            None,
        )
        .unwrap();
    assert_eq!(
        run.results()["specguard"].code,
        EligibilityCode::InvalidApproval
    );
    assert_eq!(
        run.results()["specguard"].technical_decision,
        Some(guardengine::Decision::RequireApproval)
    );
    f.authority
        .approvals
        .get_mut("fixture:actual-spec-review")
        .unwrap()
        .validity
        .revoked = false;
    let digest = guardengine::digest_json(&f.envelopes["archguard"]).unwrap();
    f.authority.records.get_mut(&digest).unwrap().principal = "forged-issuer".into();
    assert_eq!(
        prepared
            .consume(
                &f.repo,
                &f.candidate,
                &f.envelopes,
                &f.bytes,
                &f.authority,
                NOW,
                None
            )
            .unwrap()
            .results()["archguard"]
            .code,
        EligibilityCode::UnauthorizedProducer
    );
    f.authority.records.get_mut(&digest).unwrap().principal = "fixture-ci".into();
    // Recomputing all public hashes cannot create the missing authenticated producer record.
    let envelope = f.envelopes.get_mut("archguard").unwrap();
    envelope.run_id = "attacker-rehashed-envelope".into();
    assert_ne!(guardengine::digest_json(envelope).unwrap(), digest);
    assert_eq!(
        prepared
            .consume(
                &f.repo,
                &f.candidate,
                &f.envelopes,
                &f.bytes,
                &f.authority,
                NOW,
                None
            )
            .unwrap()
            .results()["archguard"]
            .code,
        EligibilityCode::UntrustedProducer
    );
}

#[test]
fn baseline_authority_contract_and_profiles_are_explicit_and_frozen() {
    for invalid in [
        "missing",
        "different-baseline",
        "different-action",
        "native",
        "duplicate",
        "pr-head",
    ] {
        let mut f = setup();
        let sg = f.required.iter_mut().find(|r| r.id == "specguard").unwrap();
        match invalid {
            "missing" => sg.baseline = None,
            "different-baseline" => {
                sg.baseline.as_mut().unwrap().policy.binding.baseline_digest = None
            }
            "different-action" => sg.baseline.as_mut().unwrap().policy.action = "push".into(),
            "native" => sg.profile = EvidenceProfile::NativeOnly,
            "duplicate" => sg.id = "archguard".into(),
            _ => sg.policy.binding.candidate_oid = sg.policy.binding.base_oid.clone(),
        }
        assert!(
            PreparedSpecialistConsumption::freeze(&f.repo, &f.candidate, f.required).is_err(),
            "{invalid}"
        );
    }
}

#[test]
fn exact_artifact_refs_no_uri_fetch_and_no_approval_repair_for_partial_or_error() {
    let mut f = setup();
    let prepared =
        PreparedSpecialistConsumption::freeze(&f.repo, &f.candidate, f.required).unwrap();
    let original = f.envelopes["specguard"].clone();
    f.envelopes
        .get_mut("specguard")
        .unwrap()
        .artifacts
        .report
        .as_mut()
        .unwrap()
        .uri = "https://never-fetched.invalid/report".into();
    assert!(
        prepared
            .consume(
                &f.repo,
                &f.candidate,
                &f.envelopes,
                &f.bytes,
                &f.authority,
                NOW,
                None
            )
            .is_err()
    );
    for status in [RunStatus::Error, RunStatus::Cancelled] {
        f.envelopes.insert("specguard".into(), original.clone());
        let envelope = f.envelopes.get_mut("specguard").unwrap();
        envelope.run_status = status;
        envelope.decision = None;
        assert!(
            !prepared
                .consume(
                    &f.repo,
                    &f.candidate,
                    &f.envelopes,
                    &f.bytes,
                    &f.authority,
                    NOW,
                    None
                )
                .unwrap()
                .eligible()
        );
    }
    f.envelopes.insert("specguard".into(), original);
    f.envelopes.get_mut("specguard").unwrap().coverage.status = CoverageStatus::Partial;
    assert!(
        !prepared
            .consume(
                &f.repo,
                &f.candidate,
                &f.envelopes,
                &f.bytes,
                &f.authority,
                NOW,
                None
            )
            .unwrap()
            .eligible()
    );
    let reference = f.envelopes["archguard"].artifacts.report.clone().unwrap();
    assert!(ImmutableArtifactBytes::new(vec![(reference, b"{}".to_vec())]).is_err());
}

#[test]
fn real_technical_block_cannot_be_repaired_by_approval() {
    use common::evidence::*;
    let e = evidence(guardengine::Enforcement::Enforce, false, false);
    assert_eq!(
        e.bundle.envelope.decision,
        Some(guardengine::Decision::Block)
    );
    let policy = policy(&e.bundle);
    let authority = FixtureAuthority::new(policy.clone());
    let prepared = PreparedSpecialistConsumption::freeze(
        &e.repo,
        &e.candidate,
        vec![RequiredSpecialist {
            id: "gitguard".into(),
            policy,
            profile: EvidenceProfile::EngineBacked,
            baseline: None,
        }],
    )
    .unwrap();
    let artifacts = [
        (&e.bundle.envelope.artifacts.contract, &e.bundle.contract),
        (&e.bundle.envelope.artifacts.facts, &e.bundle.facts),
        (&e.bundle.envelope.artifacts.report, &e.bundle.report),
    ]
    .into_iter()
    .map(|(reference, value)| {
        (
            reference.clone().unwrap(),
            serde_json::to_vec(value.as_ref().unwrap()).unwrap(),
        )
    })
    .collect();
    let bytes = ImmutableArtifactBytes::new(artifacts).unwrap();
    let run = prepared
        .consume(
            &e.repo,
            &e.candidate,
            &BTreeMap::from([("gitguard".into(), e.bundle.envelope.clone())]),
            &bytes,
            &authority,
            common::evidence::NOW,
            None,
        )
        .unwrap();
    assert!(!run.eligible());
    assert_eq!(
        run.results()["gitguard"].code,
        EligibilityCode::TechnicalBlock
    );
}

#[test]
fn freshly_recomputed_report_and_digest_are_not_producer_authentication() {
    let mut f = setup();
    let prepared =
        PreparedSpecialistConsumption::freeze(&f.repo, &f.candidate, f.required).unwrap();
    let contract = guardengine::load_contract_yaml(&fixture("archguard/contract.json")).unwrap();
    let facts = guardengine::load_facts_json(&fixture("archguard/facts.json")).unwrap();
    let report = guardengine::evaluate(&contract, &facts).unwrap();
    let recomputed = serde_json::to_vec(&report).unwrap();
    assert_ne!(recomputed, fixture("archguard/report.json"));
    use sha2::{Digest, Sha256};
    f.envelopes
        .get_mut("archguard")
        .unwrap()
        .artifacts
        .report
        .as_mut()
        .unwrap()
        .digest = format!("sha256:{:x}", Sha256::digest(&recomputed));
    let mut artifacts = vec![];
    for name in PROVIDERS {
        for (key, reference) in [
            ("contract", f.envelopes[name].artifacts.contract.as_ref()),
            ("facts", f.envelopes[name].artifacts.facts.as_ref()),
            ("report", f.envelopes[name].artifacts.report.as_ref()),
        ] {
            let bytes = if name == "archguard" && key == "report" {
                recomputed.clone()
            } else {
                fixture(&format!("{name}/{key}.json"))
            };
            artifacts.push((reference.unwrap().clone(), bytes));
        }
    }
    let bytes = ImmutableArtifactBytes::new(artifacts).unwrap();
    let run = prepared
        .consume(
            &f.repo,
            &f.candidate,
            &f.envelopes,
            &bytes,
            &f.authority,
            NOW,
            None,
        )
        .unwrap();
    assert_eq!(
        run.results()["archguard"].technical_decision,
        Some(guardengine::Decision::Allow)
    );
    assert_eq!(
        run.results()["archguard"].code,
        EligibilityCode::UntrustedProducer
    );
    assert!(!run.eligible());
}

#[test]
fn valid_partial_recomputation_remains_incomplete_despite_current_approvals() {
    let mut f = setup();
    let prepared =
        PreparedSpecialistConsumption::freeze(&f.repo, &f.candidate, f.required).unwrap();
    let contract = guardengine::load_contract_yaml(&fixture("specguard/contract.json")).unwrap();
    let mut facts = guardengine::load_facts_json(&fixture("specguard/facts.json")).unwrap();
    facts.completeness = guardengine::Completeness::Partial;
    facts
        .diagnostics
        .push("fixture: deliberately unavailable source".into());
    let report = guardengine::evaluate(&contract, &facts).unwrap();
    let changed_facts = serde_json::to_vec(&facts).unwrap();
    let changed_report = serde_json::to_vec(&report).unwrap();
    use sha2::{Digest, Sha256};
    let envelope = f.envelopes.get_mut("specguard").unwrap();
    envelope.artifacts.facts.as_mut().unwrap().digest =
        format!("sha256:{:x}", Sha256::digest(&changed_facts));
    envelope.artifacts.report.as_mut().unwrap().digest =
        format!("sha256:{:x}", Sha256::digest(&changed_report));
    envelope.decision = Some(report.decision);
    envelope.coverage.status = CoverageStatus::Partial;
    envelope.coverage.missing_scopes = vec![envelope.coverage.observed_scopes.remove(0)];
    envelope.validate(EvidenceProfile::EngineBacked).unwrap();
    let mut artifacts = vec![];
    for name in PROVIDERS {
        for (key, reference) in [
            ("contract", f.envelopes[name].artifacts.contract.as_ref()),
            ("facts", f.envelopes[name].artifacts.facts.as_ref()),
            ("report", f.envelopes[name].artifacts.report.as_ref()),
        ] {
            let bytes = match (name, key) {
                ("specguard", "facts") => changed_facts.clone(),
                ("specguard", "report") => changed_report.clone(),
                _ => fixture(&format!("{name}/{key}.json")),
            };
            artifacts.push((reference.unwrap().clone(), bytes));
        }
    }
    let bytes = ImmutableArtifactBytes::new(artifacts).unwrap();
    let run = prepared
        .consume(
            &f.repo,
            &f.candidate,
            &f.envelopes,
            &bytes,
            &f.authority,
            NOW,
            None,
        )
        .unwrap();
    assert_eq!(run.results()["specguard"].code, EligibilityCode::Incomplete);
    assert!(!run.eligible());
}
