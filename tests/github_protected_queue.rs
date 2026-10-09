use gitguard::github::*;
#[test]
fn official_hmac_vector_authenticates_exact_raw_bytes() {
    let key = WebhookKey::new(b"It's a Secret to Everybody").unwrap();
    let signature = "sha256=757107ea0eb2509fc211221cce984b8a37570b6d7586c22c46f4379c8b043e17";
    assert_eq!(key.verify_signature(b"Hello, World!", signature), Ok(()));
    assert_eq!(
        key.verify_signature(b"Hello, World?", signature),
        Err(Error::Signature)
    );
    assert!(key.verify_signature(b"Hello, World!", "").is_err());
}
mod common;
use common::{Repo, git};
use gitguard::{Repository, scope::TaskScope};
use hmac::{Hmac, Mac};
use serde_json::{Value, json};
use sha2::Sha256;
use std::collections::BTreeSet;
const SECRET: &[u8] = b"fixture-only-github-webhook-secret";
fn signed(raw: &[u8]) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(SECRET).unwrap();
    mac.update(raw);
    format!("sha256={:x}", mac.finalize().into_bytes())
}
fn config() -> Config {
    Config {
        repository_id: 17,
        installation_id: 29,
        repo_id: "repo".into(),
        base_refs: BTreeSet::from(["refs/heads/main".into()]),
    }
}
fn payload(head: &str, base: &str, tree: &str) -> Value {
    json!({"action":"checks_requested","repository":{"id":17},"installation":{"id":29,"node_id":"fixture-installation"},"merge_group":{"head_sha":head,"base_sha":base,"head_ref":"refs/heads/gh-readonly-queue/main/fixture","base_ref":"refs/heads/main","head_commit":{"id":head,"tree_id":tree,"message":"测试 UTF-8","timestamp":"2026-10-09T00:00:00Z","author":null,"committer":null}},"sender":{"login":"not-authority"},"unknown_metadata":{"future":true}})
}
fn event(v: &Value) -> VerifiedMergeGroup {
    let raw = serde_json::to_vec(v).unwrap();
    verify(
        &config(),
        &WebhookKey::new(SECRET).unwrap(),
        &raw,
        "merge_group",
        &signed(&raw),
    )
    .unwrap()
}
fn mapping(head: &str, base: &str, member: &str, generation: u64) -> Mapping {
    Mapping {
        current: ControllerCurrent {
            identity: GroupIdentity {
                repository_id: 17,
                installation_id: 29,
                head_sha: head.into(),
                base_sha: base.into(),
                head_ref: "refs/heads/gh-readonly-queue/main/fixture".into(),
                base_ref: "refs/heads/main".into(),
            },
            group_id: format!("controller:group-{generation}"),
            generation,
        },
        scope: TaskScope::advisory(
            "task",
            vec!["R".into()],
            vec![b"a".to_vec()],
            &"a".repeat(64),
            None,
        )
        .unwrap(),
        worktree_id: "w".into(),
        members: vec![member.into()],
    }
}
fn fixture() -> (Repo, String, String, String) {
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
            "signed local merge-group fixture",
        ],
    );
    (r, member, tree, head)
}
#[test]
fn signed_official_shape_maps_exact_local_commit_without_synthesizing_members() {
    let (r, member, tree, head) = fixture();
    let before = r.state();
    let repo = Repository::discover(r.path(), "repo").unwrap();
    let e = event(&payload(&head, &r.base, &tree));
    let mut m = mapping(&head, &r.base, &member, 1);
    let mapped = map_candidate(&e, &repo, &m).unwrap();
    assert_eq!(mapped.candidate().candidate_oid(), head);
    assert_eq!(mapped.candidate().base_oid(), r.base);
    assert_eq!(mapped.candidate().members(), std::slice::from_ref(&member));
    assert!(mapped.candidate().advisory());
    m.members.clear();
    assert!(map_candidate(&e, &repo, &m).is_err());
    m.members = vec![member];
    let wrong_tree = event(&payload(&head, &r.base, &"b".repeat(40)));
    assert!(map_candidate(&wrong_tree, &repo, &m).is_err());
    m.current.identity.head_sha = r.base.clone();
    assert!(map_candidate(&e, &repo, &m).is_err());
    assert_eq!(r.state(), before);
    let raw = serde_json::to_vec(&payload(&head, &r.base, &tree)).unwrap();
    println!(
        "{}",
        json!({"profile":"signed-local-github-shaped-fixture","body":String::from_utf8(raw.clone()).unwrap(),
        "signature":signed(&raw),"body_digest":e.body_digest(),"candidate":mapped.candidate(),"source_unchanged":true,"hosted_admission":false})
    );
}
#[test]
fn signed_malformed_or_wrong_controller_binding_fails_closed() {
    let (r, _, tree, head) = fixture();
    let valid = payload(&head, &r.base, &tree);
    assert_eq!(event(&valid).action(), Action::ChecksRequested);
    let key = WebhookKey::new(SECRET).unwrap();
    for pointer in [
        "/repository/id",
        "/installation/id",
        "/merge_group/base_ref",
        "/merge_group/head_commit/id",
    ] {
        let mut v = valid.clone();
        *v.pointer_mut(pointer).unwrap() =
            if pointer.ends_with("/id") && !pointer.contains("head_commit") {
                json!(99)
            } else {
                json!("wrong")
            };
        let raw = serde_json::to_vec(&v).unwrap();
        assert!(verify(&config(), &key, &raw, "merge_group", &signed(&raw)).is_err());
    }
    let raw = serde_json::to_vec(&valid).unwrap();
    let mut duplicate = b"{\"action\":\"destroyed\",".to_vec();
    duplicate.extend_from_slice(&raw[1..]);
    assert!(
        verify(
            &config(),
            &key,
            &duplicate,
            "merge_group",
            &signed(&duplicate)
        )
        .is_err()
    );
    let other = WebhookKey::new(b"different-fixture-only-key").unwrap();
    assert!(verify(&config(), &other, &raw, "merge_group", &signed(&raw)).is_err());
    let invalid_utf8 = b"{\"message\":\xff}";
    assert!(
        verify(
            &config(),
            &key,
            invalid_utf8,
            "merge_group",
            &signed(invalid_utf8)
        )
        .is_err()
    );
    assert!(verify(&config(), &key, &raw, "pull_request", &signed(&raw)).is_err());
    assert!(verify(&config(), &key, &raw, "merge_group", "sha256=00").is_err());
    assert!(key.verify_signature(&vec![b'x'; MAX_BODY + 1], "").is_err());
    let mut missing = valid.clone();
    missing["merge_group"]
        .as_object_mut()
        .unwrap()
        .remove("head_commit");
    let raw = serde_json::to_vec(&missing).unwrap();
    assert!(verify(&config(), &key, &raw, "merge_group", &signed(&raw)).is_err());
}
struct Lookup(Option<ControllerCurrent>);
impl CurrentLookup for Lookup {
    fn current(&self, _: u64, _: u64, _: &str) -> Result<Option<ControllerCurrent>, Error> {
        Ok(self.0.clone())
    }
}
#[test]
fn replayed_body_and_delayed_group_never_restore_currentness() {
    let (r, member, tree, head) = fixture();
    let later = git(
        r.path(),
        &["commit-tree", &tree, "-p", &head, "-m", "later group"],
    );
    let repo = Repository::discover(r.path(), "repo").unwrap();
    let e1 = event(&payload(&head, &r.base, &tree));
    let e2 = event(&payload(&later, &r.base, &tree));
    let m1 = mapping(&head, &r.base, &member, 1);
    let m2 = mapping(&later, &r.base, &member, 2);
    let a = map_candidate(&e1, &repo, &m1).unwrap();
    let b = map_candidate(&e2, &repo, &m2).unwrap();
    let mut replay = ReplayGuard::default();
    assert_eq!(replay.is_current(&a, &Lookup(None)), Err(Error::Unobserved));
    replay.record(&e1, "delivery-1").unwrap();
    assert_eq!(replay.record(&e2, "delivery-1"), Err(Error::Replay));
    assert!(
        replay
            .is_current(&a, &Lookup(Some(m1.current.clone())))
            .unwrap()
    );
    replay.record(&e2, "delivery-2").unwrap();
    let current = Lookup(Some(m2.current.clone()));
    assert!(!replay.is_current(&a, &current).unwrap());
    assert!(replay.is_current(&b, &current).unwrap());
    struct Unavailable;
    impl CurrentLookup for Unavailable {
        fn current(&self, _: u64, _: u64, _: &str) -> Result<Option<ControllerCurrent>, Error> {
            Err(Error::Unavailable)
        }
    }
    assert_eq!(replay.is_current(&b, &Unavailable), Err(Error::Unavailable));
    assert!(!replay.is_current(&b, &Lookup(None)).unwrap());
    assert_eq!(
        replay.record(&e1, "different-unsigned-delivery-header"),
        Err(Error::Replay)
    );
    let mut old_destroyed = payload(&head, &r.base, &tree);
    old_destroyed["action"] = json!("destroyed");
    old_destroyed["reason"] = json!("invalidated");
    let destroyed = event(&old_destroyed);
    replay.record(&destroyed, "old-destroyed").unwrap();
    assert!(map_candidate(&destroyed, &repo, &m1).is_err());
    assert!(replay.is_current(&b, &current).unwrap());
    assert!(!replay.is_current(&a, &Lookup(Some(m1.current))).unwrap());
    let mut current_destroyed = payload(&later, &r.base, &tree);
    current_destroyed["action"] = json!("destroyed");
    replay
        .record(&event(&current_destroyed), "current-destroyed")
        .unwrap();
    assert!(!replay.is_current(&b, &current).unwrap());
    assert!(!replay.is_current(&b, &Lookup(None)).unwrap());
}

#[test]
fn replay_memory_exhaustion_rejects_without_evicting_old_body() {
    let mut guard = ReplayGuard::default();
    let mut v = payload(&"a".repeat(40), &"b".repeat(40), &"c".repeat(40));
    let first = event(&v);
    guard.record(&first, "first").unwrap();
    for i in 1..256 {
        v["unknown_metadata"] = json!(i);
        guard.record(&event(&v), &format!("delivery-{i}")).unwrap();
    }
    v["unknown_metadata"] = json!(256);
    assert_eq!(guard.record(&event(&v), "overflow"), Err(Error::Budget));
    assert_eq!(guard.record(&first, "new-header"), Err(Error::Replay));
}
