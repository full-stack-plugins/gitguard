mod common;
use common::evidence::*;
use gitguard::evidence::{
    consume::{ConsumptionContext, consume, consume_bound},
    freshness::ReuseKey,
    store::{LocalHistory, StorageRequirement},
};
use guardengine::{Enforcement, integration::attempt_store::Target};
use std::sync::{Arc, Barrier};
fn key(e: &Evidence) -> ReuseKey {
    ReuseKey::new(
        &e.repo,
        &e.candidate,
        &policy(&e.bundle),
        &format!("sha256:{}", "a".repeat(64)),
        &format!("sha256:{}", "b".repeat(64)),
    )
    .unwrap()
}
#[test]
fn newer_same_binding_attempt_defeats_late_allow_and_other_task_cannot_cross_satisfy() {
    let mut e = evidence(Enforcement::Advise, false, false);
    let k = key(&e);
    let store = LocalHistory::new(StorageRequirement::ProcessLocal).unwrap();
    let p = policy(&e.bundle);
    let authority = FixtureAuthority::new(p.clone());
    let old = store.begin(&k, 0, &e.bundle.envelope.run_id).unwrap();
    let old_result = consume_bound(
        &e.repo,
        &e.candidate,
        &e.bundle,
        &p,
        &authority,
        ConsumptionContext {
            key: &k,
            now: NOW,
            cause: None,
        },
    )
    .unwrap();
    e.bundle.envelope.run_id = "retry-run".into();
    let new = store.begin(&k, 1, "retry-run").unwrap();
    let mut revoked = FixtureAuthority::new(p.clone());
    revoked.revoked = true;
    let blocked = consume_bound(
        &e.repo,
        &e.candidate,
        &e.bundle,
        &p,
        &revoked,
        ConsumptionContext {
            key: &k,
            now: NOW,
            cause: None,
        },
    )
    .unwrap();
    assert!(!blocked.result().eligible);
    store.complete(&new, &blocked).unwrap();
    store.publish(&new).unwrap();
    store.complete(&old, &old_result).unwrap();
    assert!(store.publish(&old).is_err());
    let target = Target {
        repo_id: "repo".into(),
        task_id: "task".into(),
        requirement_ids: vec!["R".into()],
    };
    assert!(!store.current(&target).unwrap().unwrap().eligible);
    assert_eq!(store.history(&target).unwrap().len(), 2);
    let other = Target {
        repo_id: "repo".into(),
        task_id: "other-task".into(),
        requirement_ids: vec!["OTHER".into()],
    };
    assert!(store.current(&other).unwrap().is_none());
    assert!(store.begin(&k, 2, "retry-run").is_err());
}
#[test]
fn two_threads_cas_one_winner_and_durability_requirement_is_rejected() {
    assert!(LocalHistory::new(StorageRequirement::Durable).is_err());
    let e = evidence(Enforcement::Advise, false, false);
    let key = key(&e);
    let store = Arc::new(LocalHistory::new(StorageRequirement::ProcessLocal).unwrap());
    let barrier = Arc::new(Barrier::new(2));
    let handles = (0..2)
        .map(|i| {
            let store = store.clone();
            let barrier = barrier.clone();
            let key = key.clone();
            std::thread::spawn(move || {
                barrier.wait();
                store.begin(&key, 0, &format!("run-{i}")).is_ok()
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        handles
            .into_iter()
            .map(|h| usize::from(h.join().unwrap()))
            .sum::<usize>(),
        1
    );
}
#[test]
fn tickets_from_another_store_cannot_publish_same_named_attempt() {
    let e = evidence(Enforcement::Advise, false, false);
    let k = key(&e);
    let a = LocalHistory::new(StorageRequirement::ProcessLocal).unwrap();
    let b = LocalHistory::new(StorageRequirement::ProcessLocal).unwrap();
    let ta = a.begin(&k, 0, &e.bundle.envelope.run_id).unwrap();
    let tb = b.begin(&k, 0, &e.bundle.envelope.run_id).unwrap();
    let p = policy(&e.bundle);
    let result = consume_bound(
        &e.repo,
        &e.candidate,
        &e.bundle,
        &p,
        &FixtureAuthority::new(p.clone()),
        ConsumptionContext {
            key: &k,
            now: NOW,
            cause: None,
        },
    )
    .unwrap();
    b.complete(&tb, &result).unwrap();
    assert!(b.publish(&ta).is_err());
    assert!(b.complete(&ta, &result).is_err());
}
#[test]
fn two_active_requirements_keep_independent_history_and_completion() {
    use gitguard::{
        candidate::{CandidateRequest, CandidateSnapshot},
        cli::CheckRequest,
        evidence::{envelope::BoundCheck, projection::FrozenPolicy},
        scope::TaskScope,
    };
    use std::sync::atomic::AtomicBool;
    let first = evidence(Enforcement::Advise, false, false);
    let frozen = FrozenPolicy::new(Enforcement::Advise);
    let second = BoundCheck::prepare(CheckRequest {
        api_version: "gitguard.check/v1alpha1".into(),
        repo_root: first.source.path().into(),
        repo_id: "repo".into(),
        candidate_oid: first.candidate.candidate_oid().into(),
        scope: TaskScope::advisory(
            "second-task",
            vec!["SECOND".into()],
            vec![b"other".to_vec()],
            &frozen.digest(),
            None,
        )
        .unwrap(),
        candidate: CandidateRequest {
            worktree_id: "second-worktree".into(),
            base_oid: first.source.base.clone(),
            merge_group_id: None,
            members: vec![],
        },
        contract: frozen.contract(),
    })
    .unwrap()
    .run(&AtomicBool::new(false))
    .unwrap();
    let second_candidate: CandidateSnapshot =
        serde_json::from_value(second.domain["candidate"].clone()).unwrap();
    let first_key = key(&first);
    let second_policy = policy(&second);
    let second_key = ReuseKey::new(
        &first.repo,
        &second_candidate,
        &second_policy,
        &format!("sha256:{}", "a".repeat(64)),
        &format!("sha256:{}", "b".repeat(64)),
    )
    .unwrap();
    let store = LocalHistory::new(StorageRequirement::ProcessLocal).unwrap();
    let t1 = store
        .begin(&first_key, 0, &first.bundle.envelope.run_id)
        .unwrap();
    let t2 = store
        .begin(&second_key, 0, &second.envelope.run_id)
        .unwrap();
    let p1 = policy(&first.bundle);
    let r1 = consume_bound(
        &first.repo,
        &first.candidate,
        &first.bundle,
        &p1,
        &FixtureAuthority::new(p1.clone()),
        ConsumptionContext {
            key: &first_key,
            now: NOW,
            cause: None,
        },
    )
    .unwrap();
    let r2 = consume_bound(
        &first.repo,
        &second_candidate,
        &second,
        &second_policy,
        &FixtureAuthority::new(second_policy.clone()),
        ConsumptionContext {
            key: &second_key,
            now: NOW,
            cause: None,
        },
    )
    .unwrap();
    assert!(store.complete(&t2, &r1).is_err());
    store.complete(&t1, &r1).unwrap();
    store.complete(&t2, &r2).unwrap();
    store.publish(&t1).unwrap();
    store.publish(&t2).unwrap();
    for (task, requirement, run) in [
        ("task", "R", first.bundle.envelope.run_id),
        ("second-task", "SECOND", second.envelope.run_id),
    ] {
        let target = Target {
            repo_id: "repo".into(),
            task_id: task.into(),
            requirement_ids: vec![requirement.into()],
        };
        assert_eq!(store.history(&target).unwrap().len(), 1);
        assert_eq!(store.current(&target).unwrap().unwrap().run_id, run);
    }
}

#[test]
fn review_completion_cannot_relabel_result_under_changed_dependency_key() {
    let e = evidence(Enforcement::Advise, false, false);
    let p = policy(&e.bundle);
    let authority = FixtureAuthority::new(p.clone());
    let old = consume(&e.repo, &e.candidate, &e.bundle, &p, &authority, NOW, None).unwrap();
    let new_key = ReuseKey::new(
        &e.repo,
        &e.candidate,
        &p,
        &format!("sha256:{}", "c".repeat(64)),
        &format!("sha256:{}", "d".repeat(64)),
    )
    .unwrap();
    assert!(!key(&e).same_inputs(&new_key));
    let store = LocalHistory::new(StorageRequirement::ProcessLocal).unwrap();
    let ticket = store.begin(&new_key, 0, &e.bundle.envelope.run_id).unwrap();
    assert!(
        store.complete(&ticket, &old).is_err(),
        "old consumption must not be recorded under changed dependency/config identity"
    );
}

#[test]
fn bound_result_cannot_move_between_dependency_or_configuration_keys() {
    let e = evidence(Enforcement::Advise, false, false);
    let p = policy(&e.bundle);
    let authority = FixtureAuthority::new(p.clone());
    let original_key = key(&e);
    let original = consume_bound(
        &e.repo,
        &e.candidate,
        &e.bundle,
        &p,
        &authority,
        ConsumptionContext {
            key: &original_key,
            now: NOW,
            cause: None,
        },
    )
    .unwrap();
    for (dependency, configuration) in [("c", "b"), ("a", "d")] {
        let changed_key = ReuseKey::new(
            &e.repo,
            &e.candidate,
            &p,
            &format!("sha256:{}", dependency.repeat(64)),
            &format!("sha256:{}", configuration.repeat(64)),
        )
        .unwrap();
        let store = LocalHistory::new(StorageRequirement::ProcessLocal).unwrap();
        let ticket = store
            .begin(&changed_key, 0, &e.bundle.envelope.run_id)
            .unwrap();
        assert!(store.complete(&ticket, &original).is_err());
        let fresh = consume_bound(
            &e.repo,
            &e.candidate,
            &e.bundle,
            &p,
            &authority,
            ConsumptionContext {
                key: &changed_key,
                now: NOW,
                cause: None,
            },
        )
        .unwrap();
        store.complete(&ticket, &fresh).unwrap();
        store.publish(&ticket).unwrap();
    }
    let mut wrong_policy = p.clone();
    wrong_policy.action = "other-action".into();
    assert!(
        consume_bound(
            &e.repo,
            &e.candidate,
            &e.bundle,
            &wrong_policy,
            &authority,
            ConsumptionContext {
                key: &original_key,
                now: NOW,
                cause: None
            }
        )
        .is_err()
    );
}
