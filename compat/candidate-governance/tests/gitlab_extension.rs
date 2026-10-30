mod common;
use common::{Repo, git};
use gitguard::{Repository, gitlab::*, scope::TaskScope};
use serde_json::{Value, json};
use std::cell::RefCell;
struct Port(RefCell<Option<CurrentTrain>>);
impl CurrentLookup for Port {
    fn current(&self, _: u64, _: u64) -> Result<Option<CurrentTrain>, Error> {
        Ok(self.0.borrow().clone())
    }
}
fn fixture() -> (Repo, Value, CurrentTrain, TaskScope) {
    let r = Repo::new("sha1");
    let member = r.commit("a", "member");
    let tree = git(r.path(), &["rev-parse", &format!("{member}^{{tree}}")]);
    let head = git(
        r.path(),
        &[
            "commit-tree",
            &tree,
            "-p",
            &r.base,
            "-p",
            &member,
            "-m",
            "local train fixture",
        ],
    );
    let body = json!({"id":110,"target_branch":"main","status":"fresh","merge_request":{"id":126,"iid":59,"project_id":20,"state":"opened"},"pipeline":{"id":246,"project_id":20,"sha":head,"ref":"refs/merge-requests/59/train","source":"merge_request_event","status":"success"}});
    let e = CurrentTrain {
        profile: PROFILE.into(),
        repo_id: "repo".into(),
        project_id: 20,
        mr_id: 126,
        mr_iid: 59,
        train_id: 110,
        pipeline_id: 246,
        target_branch: "main".into(),
        pipeline_ref: "refs/merge-requests/59/train".into(),
        candidate_oid: head,
        base_oid: r.base.clone(),
        members: vec![member],
        group_id: "controller:train".into(),
        generation: 1,
        response_digest: response_digest(&serde_json::to_vec(&body).unwrap()),
        merge_train_pipeline: true,
    };
    let scope = TaskScope::advisory(
        "task",
        vec!["R".into()],
        vec![b"a".to_vec()],
        &"a".repeat(64),
        None,
    )
    .unwrap();
    (r, body, e, scope)
}
#[test]
fn exact_gitlab_candidate_and_fresh_controller_required() {
    let (r, b, e, s) = fixture();
    let before = r.state();
    let repo = Repository::discover(r.path(), "repo").unwrap();
    let port = Port(RefCell::new(Some(e.clone())));
    let raw = serde_json::to_vec(&b).unwrap();
    let p = prepare(&raw, &repo, &e, &s, "w", &port).unwrap();
    assert_eq!(p.candidate().candidate_oid(), e.candidate_oid);
    assert_eq!(p.candidate().members(), e.members);
    assert!(p.candidate().advisory());
    assert!(p.is_current(&port).unwrap());
    port.0.borrow_mut().as_mut().unwrap().generation += 1;
    assert!(!p.is_current(&port).unwrap());
    assert!(prepare(&raw, &repo, &e, &s, "w", &port).is_err());
    *port.0.borrow_mut() = None;
    assert!(!p.is_current(&port).unwrap());
    assert!(prepare(&raw, &repo, &e, &s, "w", &port).is_err());
    assert_eq!(r.state(), before);
}
#[test]
fn nullable_fallback_unknown_and_mismatched_gitlab_responses_fail() {
    let (r, b, e, s) = fixture();
    let repo = Repository::discover(r.path(), "repo").unwrap();
    for (pointer, value) in [
        ("/pipeline", Value::Null),
        ("/status", json!("stale")),
        ("/pipeline/status", json!("future")),
        ("/pipeline/source", json!("push")),
        ("/pipeline/project_id", json!(21)),
        ("/merge_request/iid", json!(60)),
        ("/target_branch", json!("other")),
        ("/pipeline/ref", json!("refs/merge-requests/59/head")),
        ("/pipeline/sha", json!("0".repeat(40))),
    ] {
        let mut v = b.clone();
        *v.pointer_mut(pointer).unwrap() = value;
        let raw = serde_json::to_vec(&v).unwrap();
        let mut current = e.clone();
        current.response_digest = response_digest(&raw);
        let port = Port(RefCell::new(Some(current.clone())));
        assert!(
            prepare(&raw, &repo, &current, &s, "w", &port).is_err(),
            "{pointer}"
        );
    }
    for variant in 0..5 {
        let mut current = e.clone();
        match variant {
            0 => current.profile = "unknown".into(),
            1 => current.merge_train_pipeline = false,
            2 => current.members.clear(),
            3 => current.candidate_oid = current.members[0].clone(),
            _ => current.generation = 0,
        };
        let port = Port(RefCell::new(Some(current.clone())));
        assert!(
            prepare(
                &serde_json::to_vec(&b).unwrap(),
                &repo,
                &current,
                &s,
                "w",
                &port
            )
            .is_err()
        );
    }
    let port = Port(RefCell::new(Some(e.clone())));
    for raw in [
        b"{\"id\":1,\"id\":2}".to_vec(),
        vec![b' '; 1_048_577],
        b"[]".to_vec(),
    ] {
        assert!(prepare(&raw, &repo, &e, &s, "w", &port).is_err());
    }
}

#[test]
fn controller_cannot_replace_missing_git_objects_and_lookup_failure_is_closed() {
    let (r, mut b, mut e, s) = fixture();
    let repo = Repository::discover(r.path(), "repo").unwrap();
    e.candidate_oid = "b".repeat(40);
    b["pipeline"]["sha"] = json!(e.candidate_oid);
    let raw = serde_json::to_vec(&b).unwrap();
    e.response_digest = response_digest(&raw);
    let port = Port(RefCell::new(Some(e.clone())));
    assert!(matches!(
        prepare(&raw, &repo, &e, &s, "w", &port),
        Err(Error::Mapping)
    ));
    struct Missing;
    impl CurrentLookup for Missing {
        fn current(&self, _: u64, _: u64) -> Result<Option<CurrentTrain>, Error> {
            Err(Error::Unavailable)
        }
    }
    assert!(matches!(
        prepare(&raw, &repo, &e, &s, "w", &Missing),
        Err(Error::Unavailable)
    ));
    let (_, b, e, _) = fixture();
    struct Changing(RefCell<u32>, CurrentTrain);
    impl CurrentLookup for Changing {
        fn current(&self, _: u64, _: u64) -> Result<Option<CurrentTrain>, Error> {
            let mut calls = self.0.borrow_mut();
            *calls += 1;
            Ok(if *calls == 1 {
                Some(self.1.clone())
            } else {
                None
            })
        }
    }
    // Recreate against the same deterministic fixture object IDs.
    let port = Changing(RefCell::new(0), e.clone());
    assert!(matches!(
        prepare(&serde_json::to_vec(&b).unwrap(), &repo, &e, &s, "w", &port),
        Err(Error::Unavailable)
    ));
    assert_eq!(*port.0.borrow(), 2);
}
