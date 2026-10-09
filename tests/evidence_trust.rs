mod common;
use common::evidence::*;
use gitguard::evidence::consume::consume;
use guardengine::{Decision, Enforcement, integration::eligibility::EligibilityCode};
#[test]
fn real_artifacts_need_fresh_authority_and_exact_candidate_baseline() {
    let e = evidence(Enforcement::Advise, false, false);
    let p = policy(&e.bundle);
    let mut authority = FixtureAuthority::new(p.clone());
    let good = consume(&e.repo, &e.candidate, &e.bundle, &p, &authority, NOW, None).unwrap();
    assert!(good.result().eligible);
    authority.principal = "forged-issuer".into();
    assert_eq!(
        consume(&e.repo, &e.candidate, &e.bundle, &p, &authority, NOW, None)
            .unwrap()
            .result()
            .code,
        EligibilityCode::UnauthorizedProducer
    );
    authority.available = false;
    assert!(
        !consume(&e.repo, &e.candidate, &e.bundle, &p, &authority, NOW, None)
            .unwrap()
            .result()
            .eligible
    );
    let mut wrong = p.clone();
    wrong.binding.candidate_oid = e.source.base.clone();
    assert!(
        !consume(
            &e.repo,
            &e.candidate,
            &e.bundle,
            &wrong,
            &FixtureAuthority::new(wrong.clone()),
            NOW,
            None
        )
        .unwrap()
        .result()
        .eligible
    );
    let mut wrong = p.clone();
    wrong.binding.baseline_digest = Some(format!("sha256:{}", "b".repeat(64)));
    assert!(
        !consume(
            &e.repo,
            &e.candidate,
            &e.bundle,
            &wrong,
            &FixtureAuthority::new(wrong.clone()),
            NOW,
            None
        )
        .unwrap()
        .result()
        .eligible
    );
}
#[test]
fn approval_does_not_rewrite_review_or_repair_partial_error_and_domain_tampering() {
    let mut review = evidence(Enforcement::Review, false, false);
    review.bundle.envelope.approval_refs = vec!["fixture:approval".into()];
    let p = policy(&review.bundle);
    let authority = FixtureAuthority::new(p.clone());
    let result = consume(
        &review.repo,
        &review.candidate,
        &review.bundle,
        &p,
        &authority,
        NOW,
        None,
    )
    .unwrap();
    assert!(result.result().eligible);
    assert_eq!(
        result.result().technical_decision,
        Some(Decision::RequireApproval)
    );
    assert_eq!(
        review.bundle.envelope.decision,
        Some(Decision::RequireApproval)
    );
    review.bundle.domain["authorization"] = serde_json::json!("approved");
    assert!(
        consume(
            &review.repo,
            &review.candidate,
            &review.bundle,
            &p,
            &authority,
            NOW,
            None
        )
        .is_err()
    );
    for (dirty, cancel) in [(true, false), (false, true)] {
        let mut e = evidence(Enforcement::Review, dirty, cancel);
        e.bundle.envelope.approval_refs = vec!["fixture:approval".into()];
        let p = policy(&e.bundle);
        assert!(
            !consume(
                &e.repo,
                &e.candidate,
                &e.bundle,
                &p,
                &FixtureAuthority::new(p.clone()),
                NOW,
                None
            )
            .unwrap()
            .result()
            .eligible
        );
    }
}
#[test]
fn rehashed_unknown_domain_fields_and_missing_provider_do_not_gain_trust() {
    use gitguard::evidence::consume::UnavailableAuthority;
    use sha2::{Digest, Sha256};
    let mut e = evidence(Enforcement::Advise, false, false);
    let p = policy(&e.bundle);
    assert!(
        !consume(
            &e.repo,
            &e.candidate,
            &e.bundle,
            &p,
            &UnavailableAuthority,
            NOW,
            None
        )
        .unwrap()
        .result()
        .eligible
    );
    e.bundle.domain["approved"] = serde_json::json!(true);
    e.bundle.envelope.artifacts.domain[0].digest = format!(
        "sha256:{:x}",
        Sha256::digest(serde_json::to_vec(&e.bundle.domain).unwrap())
    );
    assert!(
        consume(
            &e.repo,
            &e.candidate,
            &e.bundle,
            &p,
            &FixtureAuthority::new(p.clone()),
            NOW,
            None
        )
        .is_err()
    );
}
