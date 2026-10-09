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
