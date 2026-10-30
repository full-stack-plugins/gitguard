mod common;
use common::evidence::*;
use gitguard::evidence::{consume::consume, freshness::ReuseKey};
use guardengine::Enforcement;
#[test]
fn every_declared_input_drift_invalidates_reuse_and_authority_is_rechecked() {
    let e = evidence(Enforcement::Advise, false, false);
    let p = policy(&e.bundle);
    let dep = format!("sha256:{}", "a".repeat(64));
    let config = format!("sha256:{}", "b".repeat(64));
    let original = ReuseKey::new(&e.repo, &e.candidate, &p, &dep, &config).unwrap();
    for change in 0..7 {
        let mut next = p.clone();
        match change {
            0 => next.binding.source_snapshot_digest = format!("sha256:{}", "c".repeat(64)),
            1 => next.binding.baseline_digest = Some(format!("sha256:{}", "c".repeat(64))),
            2 => next.contract_digest = format!("sha256:{}", "c".repeat(64)),
            3 => next.producer.analyzer_version = "next".into(),
            4 => next.required_scopes.push("new-obligation".into()),
            5 => next.binding.candidate_oid = e.source.base.clone(),
            _ => next.action = "different-action".into(),
        };
        let key = ReuseKey::new(&e.repo, &e.candidate, &next, &dep, &config).unwrap();
        assert!(!original.same_inputs(&key));
    }
    assert!(
        !original.same_inputs(&ReuseKey::new(&e.repo, &e.candidate, &p, &config, &config).unwrap())
    );
    assert!(!original.same_inputs(&ReuseKey::new(&e.repo, &e.candidate, &p, &dep, &dep).unwrap()));
    let mut authority = FixtureAuthority::new(p.clone());
    let saved = serde_json::to_vec(&e.bundle.envelope).unwrap();
    assert!(
        consume(&e.repo, &e.candidate, &e.bundle, &p, &authority, NOW, None)
            .unwrap()
            .result()
            .eligible
    );
    authority.expires = NOW;
    assert!(
        !consume(&e.repo, &e.candidate, &e.bundle, &p, &authority, NOW, None)
            .unwrap()
            .result()
            .eligible
    );
    authority.expires = i64::MAX;
    authority.revoked = true;
    assert!(
        !consume(&e.repo, &e.candidate, &e.bundle, &p, &authority, NOW, None)
            .unwrap()
            .result()
            .eligible
    );
    assert_eq!(saved, serde_json::to_vec(&e.bundle.envelope).unwrap());
}
#[test]
fn audit_is_redacted_access_controlled_and_retention_cannot_mutate_original_report() {
    use gitguard::evidence::audit::*;
    use std::collections::BTreeSet;
    struct FixtureIdentity(&'static str);
    impl AuditIdentityProvider for FixtureIdentity {
        fn authenticated_principal(&self) -> Result<String, String> {
            Ok(self.0.into())
        }
    }
    let e = evidence(Enforcement::Advise, false, false);
    let p = policy(&e.bundle);
    let original = serde_json::to_vec(&e.bundle.envelope).unwrap();
    let result = consume(
        &e.repo,
        &e.candidate,
        &e.bundle,
        &p,
        &FixtureAuthority::new(p.clone()),
        NOW,
        Some("TOKEN_SECRET_CAUSE"),
    )
    .unwrap();
    let audit = AuditLog::new(AuditPolicy {
        readers: BTreeSet::from(["reader".into()]),
        deleters: BTreeSet::from(["retention-admin".into()]),
        retention_seconds: 60,
        max_records: 4,
    })
    .unwrap();
    audit.append(&result).unwrap();
    assert!(audit.read(&FixtureIdentity("intruder")).is_err());
    let rows = audit.read(&FixtureIdentity("reader")).unwrap();
    assert_eq!(rows.len(), 1);
    let json = serde_json::to_string(&rows).unwrap();
    assert!(!json.contains("TOKEN_SECRET_CAUSE"));
    assert!(!json.contains("fixture-ci"));
    assert!(rows[0].cause_digest.is_some());
    assert!(audit.purge(&FixtureIdentity("reader"), NOW + 60).is_err());
    assert_eq!(
        audit
            .purge(&FixtureIdentity("retention-admin"), NOW + 59)
            .unwrap(),
        0
    );
    assert_eq!(
        audit
            .purge(&FixtureIdentity("retention-admin"), NOW + 60)
            .unwrap(),
        1
    );
    assert!(audit.read(&FixtureIdentity("reader")).unwrap().is_empty());
    assert_eq!(original, serde_json::to_vec(&e.bundle.envelope).unwrap());
}

#[test]
fn audit_policy_and_clock_are_bounded() {
    use gitguard::evidence::audit::*;
    use std::collections::BTreeSet;
    struct Admin;
    impl AuditIdentityProvider for Admin {
        fn authenticated_principal(&self) -> Result<String, String> {
            Ok("admin".into())
        }
    }
    let policy = AuditPolicy {
        readers: BTreeSet::from(["admin".into()]),
        deleters: BTreeSet::from(["admin".into()]),
        retention_seconds: 1,
        max_records: 10_001,
    };
    assert!(AuditLog::new(policy.clone()).is_err());
    let audit = AuditLog::new(AuditPolicy {
        max_records: 4,
        ..policy
    })
    .unwrap();
    assert!(audit.purge(&Admin, -1).is_err());
    audit.purge(&Admin, NOW).unwrap();
    assert!(audit.purge(&Admin, NOW - 1).is_err());
}
#[test]
fn oversized_policy_is_rejected_before_reuse_hash() {
    let e = evidence(Enforcement::Advise, false, false);
    let mut p = policy(&e.bundle);
    p.action = "x".repeat(1024 * 1024);
    let digest = format!("sha256:{}", "a".repeat(64));
    assert!(ReuseKey::new(&e.repo, &e.candidate, &p, &digest, &digest).is_err());
}

#[test]
fn frozen_freshness_rechecks_authority_and_rejects_each_input_and_raw_drift() {
    use gitguard::evidence::freshness::{FreshnessContext, FreshnessReason, FreshnessSession};
    let e = evidence(Enforcement::Advise, false, false);
    let p = policy(&e.bundle);
    let dep = format!("sha256:{}", "a".repeat(64));
    let config = format!("sha256:{}", "b".repeat(64));
    let context = |now| FreshnessContext {
        dependencies: &dep,
        config: &config,
        now,
        cause: Some("SECRET_CAUSE"),
    };
    let mut frozen =
        FreshnessSession::freeze(&e.repo, &e.candidate, &e.bundle, &p, context(NOW)).unwrap();
    let mut authority = FixtureAuthority::new(p.clone());
    let observation = frozen
        .observe(
            &e.repo,
            &e.candidate,
            &e.bundle,
            &p,
            &authority,
            context(NOW),
        )
        .unwrap();
    assert!(observation.consumption().unwrap().result().eligible);
    authority.revoked = true;
    assert!(
        !frozen
            .observe(
                &e.repo,
                &e.candidate,
                &e.bundle,
                &p,
                &authority,
                context(NOW)
            )
            .unwrap()
            .consumption()
            .unwrap()
            .result()
            .eligible
    );
    authority.revoked = false;
    authority.expires = NOW;
    assert!(
        !frozen
            .observe(
                &e.repo,
                &e.candidate,
                &e.bundle,
                &p,
                &authority,
                context(NOW)
            )
            .unwrap()
            .consumption()
            .unwrap()
            .result()
            .eligible
    );
    for change in 0..8 {
        let mut changed = p.clone();
        match change {
            0 => changed.binding.source_snapshot_digest = config.clone(),
            1 => changed.binding.baseline_digest = Some(config.clone()),
            2 => changed.contract_digest = config.clone(),
            3 => changed.producer.analyzer_version = "next".into(),
            4 => changed.required_scopes.push("new".into()),
            5 => changed.binding.candidate_oid = e.source.base.clone(),
            6 => changed.action = "other".into(),
            _ => changed.producer_principals.clear(),
        }
        let observed = frozen
            .observe(
                &e.repo,
                &e.candidate,
                &e.bundle,
                &changed,
                &authority,
                context(NOW),
            )
            .unwrap();
        assert_eq!(observed.audit().reason, FreshnessReason::InputsChanged);
        assert!(observed.consumption().is_none());
    }
    for (dependencies, configuration) in [(&config, &config), (&dep, &dep)] {
        let observed = frozen
            .observe(
                &e.repo,
                &e.candidate,
                &e.bundle,
                &p,
                &authority,
                FreshnessContext {
                    dependencies,
                    config: configuration,
                    ..context(NOW)
                },
            )
            .unwrap();
        assert_eq!(observed.audit().reason, FreshnessReason::InputsChanged);
    }
    let original = serde_json::to_vec(&e.bundle).unwrap();
    let mut missing: gitguard::evidence::envelope::CheckBundle =
        serde_json::from_slice(&original).unwrap();
    missing.report = None;
    let observed = frozen
        .observe(
            &e.repo,
            &e.candidate,
            &missing,
            &p,
            &authority,
            context(NOW),
        )
        .unwrap();
    assert_eq!(observed.audit().reason, FreshnessReason::EvidenceChanged);
    assert!(observed.consumption().is_none());
    assert!(
        !serde_json::to_string(observed.audit())
            .unwrap()
            .contains("SECRET_CAUSE")
    );
    assert!(
        frozen
            .observe(
                &e.repo,
                &e.candidate,
                &e.bundle,
                &p,
                &authority,
                context(NOW - 1)
            )
            .is_err()
    );
    assert_eq!(original, serde_json::to_vec(&e.bundle).unwrap());
}

#[test]
fn freshness_preserves_reports_and_requeries_approval_expiry_revocation_and_missing_attachments() {
    use gitguard::evidence::freshness::*;
    use guardengine::integration::{GuardRunEnvelope, eligibility::*};
    use std::cell::Cell;
    struct Authority {
        base: FixtureAuthority,
        expired: bool,
        revoked: bool,
        calls: Cell<usize>,
    }
    impl AuthorityProvider for Authority {
        fn verify_producer(
            &self,
            envelope: &GuardRunEnvelope,
            digest: &str,
        ) -> Result<ProducerRecord, AuthorityError> {
            self.base.verify_producer(envelope, digest)
        }
        fn verify_approval(&self, reference: &str) -> Result<ApprovalRecord, AuthorityError> {
            self.calls.set(self.calls.get() + 1);
            let mut record = self.base.verify_approval(reference)?;
            if self.expired {
                record.validity.expires_at = NOW;
            }
            record.validity.revoked = self.revoked;
            Ok(record)
        }
    }
    let mut e = evidence(Enforcement::Review, false, false);
    e.bundle.envelope.approval_refs = vec!["fixture:approval".into()];
    let p = policy(&e.bundle);
    let bytes = serde_json::to_vec(&e.bundle).unwrap();
    let digest = format!("sha256:{}", "a".repeat(64));
    let context = FreshnessContext {
        dependencies: &digest,
        config: &digest,
        now: NOW,
        cause: None,
    };
    let mut session =
        FreshnessSession::freeze(&e.repo, &e.candidate, &e.bundle, &p, context).unwrap();
    let mut authority = Authority {
        base: FixtureAuthority::new(p.clone()),
        expired: false,
        revoked: false,
        calls: Cell::new(0),
    };
    for (expired, revoked, eligible) in [
        (false, false, true),
        (true, false, false),
        (false, true, false),
        (false, false, true),
    ] {
        authority.expired = expired;
        authority.revoked = revoked;
        let observation = session
            .observe(&e.repo, &e.candidate, &e.bundle, &p, &authority, context)
            .unwrap();
        let result = observation.consumption().unwrap().result();
        assert_eq!(result.eligible, eligible);
        assert_eq!(
            result.technical_decision,
            Some(guardengine::Decision::RequireApproval)
        );
    }
    assert_eq!(authority.calls.get(), 4);
    for change in 0..6 {
        let mut bundle: gitguard::evidence::envelope::CheckBundle =
            serde_json::from_slice(&bytes).unwrap();
        match change {
            0 => bundle.envelope.approval_refs.clear(),
            1 => bundle.contract = None,
            2 => bundle.facts = None,
            3 => bundle.report = None,
            4 => bundle.domain["advisory"] = false.into(),
            _ => bundle.envelope.run_id.push_str("-retry"),
        }
        let observation = session
            .observe(&e.repo, &e.candidate, &bundle, &p, &authority, context)
            .unwrap();
        assert_eq!(observation.audit().reason, FreshnessReason::EvidenceChanged);
        assert!(observation.consumption().is_none());
    }
    assert_eq!(authority.calls.get(), 4);
    assert_eq!(bytes, serde_json::to_vec(&e.bundle).unwrap());
}

#[test]
fn audit_combines_capacity_retention_and_static_error_redaction() {
    use gitguard::evidence::{audit::*, freshness::*};
    use std::collections::BTreeSet;
    struct Identity(bool);
    impl AuditIdentityProvider for Identity {
        fn authenticated_principal(&self) -> Result<String, String> {
            if self.0 {
                Ok("admin".into())
            } else {
                Err("EXTERNAL_SECRET".into())
            }
        }
    }
    let e = evidence(Enforcement::Advise, false, false);
    let p = policy(&e.bundle);
    let original = serde_json::to_vec(&e.bundle).unwrap();
    let digest = format!("sha256:{}", "a".repeat(64));
    let context = FreshnessContext {
        dependencies: &digest,
        config: &digest,
        now: NOW,
        cause: Some("CAUSE_SECRET"),
    };
    let mut session =
        FreshnessSession::freeze(&e.repo, &e.candidate, &e.bundle, &p, context).unwrap();
    let authority = FixtureAuthority::new(p.clone());
    let observed = session
        .observe(&e.repo, &e.candidate, &e.bundle, &p, &authority, context)
        .unwrap();
    let audit = AuditLog::new(AuditPolicy {
        readers: BTreeSet::from(["admin".into()]),
        deleters: BTreeSet::from(["admin".into()]),
        retention_seconds: 1,
        max_records: 2,
    })
    .unwrap();
    audit.append_freshness(&observed).unwrap();
    audit.append(observed.consumption().unwrap()).unwrap();
    assert!(audit.append_freshness(&observed).is_err());
    assert_eq!(
        audit.read_freshness(&Identity(false)).unwrap_err(),
        "audit identity unavailable"
    );
    let serialized =
        serde_json::to_string(&audit.read_freshness(&Identity(true)).unwrap()).unwrap();
    assert!(!serialized.contains("SECRET"));
    assert!(!serialized.contains("fixture-ci"));
    assert!(audit.purge(&Identity(false), NOW + 1).is_err());
    assert_eq!(audit.purge(&Identity(true), NOW + 1).unwrap(), 2);
    assert!(audit.append_freshness(&observed).is_err());
    // Audit retention does not create a cached revocation or alter technical artifacts.
    assert!(
        session
            .observe(
                &e.repo,
                &e.candidate,
                &e.bundle,
                &p,
                &authority,
                FreshnessContext {
                    now: NOW + 1,
                    ..context
                }
            )
            .unwrap()
            .consumption()
            .unwrap()
            .result()
            .eligible
    );
    assert_eq!(original, serde_json::to_vec(&e.bundle).unwrap());
}

#[test]
fn freshness_budget_failure_and_clock_rollback_never_qualify() {
    use gitguard::evidence::freshness::*;
    let e = evidence(Enforcement::Advise, false, false);
    let p = policy(&e.bundle);
    let digest = format!("sha256:{}", "a".repeat(64));
    let context = FreshnessContext {
        dependencies: &digest,
        config: &digest,
        now: NOW,
        cause: None,
    };
    let mut session =
        FreshnessSession::freeze(&e.repo, &e.candidate, &e.bundle, &p, context).unwrap();
    let authority = FixtureAuthority::new(p.clone());
    let mut oversized: gitguard::evidence::envelope::CheckBundle =
        serde_json::from_slice(&serde_json::to_vec(&e.bundle).unwrap()).unwrap();
    oversized.domain["oversized"] = "\0".repeat(3 * 1024 * 1024).into();
    let result = session
        .observe(
            &e.repo,
            &e.candidate,
            &oversized,
            &p,
            &authority,
            FreshnessContext {
                now: NOW + 1,
                ..context
            },
        )
        .unwrap();
    assert_eq!(result.audit().reason, FreshnessReason::InvalidEvidence);
    assert!(result.consumption().is_none());
    assert!(result.audit().observed_evidence.is_none());
    assert!(
        session
            .observe(&e.repo, &e.candidate, &e.bundle, &p, &authority, context)
            .is_err()
    );
    assert!(consume(&e.repo, &e.candidate, &oversized, &p, &authority, NOW, None).is_err());
    assert!(
        FreshnessSession::freeze(
            &e.repo,
            &e.candidate,
            &e.bundle,
            &p,
            FreshnessContext { now: -1, ..context }
        )
        .is_err()
    );
    let cause = "x".repeat(16 * 1024 + 1);
    assert!(
        consume(
            &e.repo,
            &e.candidate,
            &e.bundle,
            &p,
            &authority,
            NOW,
            Some(&cause)
        )
        .is_err()
    );
}
