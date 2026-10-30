//! GG-only consumer replay of four independently captured real producer groups.
//! No producer rerun, production identity, FlowGuard dependency or implicit grant.
use gitguard::{
    Repository,
    candidate::CandidateSnapshot,
    evidence::{
        consume::{ConsumptionContext, consume_bound},
        envelope::CheckBundle,
        freshness::ReuseKey,
        specialists::*,
        store::{LocalHistory, StorageRequirement},
    },
    execution::{self, ExecutionConfig, Operation},
};
use guardengine::{
    Decision,
    integration::{eligibility::*, *},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
};
const NOW: i64 = 2_000_000_000;
const DOMAINS: [&str; 5] = [
    "archguard",
    "codeguard",
    "gitguard",
    "specguard",
    "testguard",
];
const TRANSPORT_SHA: &str = "ee01277e9d9c6550466d4bce77d8675cedcde31b443e708e6ebf926c5182c76c";
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn value(path: impl AsRef<Path>) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}
fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("/usr/bin/git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}
fn verify_corpus(root: &Path) -> Value {
    let raw = std::fs::read(root.join("TRANSPORT.json")).unwrap();
    assert_eq!(digest(&raw), TRANSPORT_SHA);
    let transport: Value = serde_json::from_slice(&raw).unwrap();
    for (name, sha) in transport["files"].as_object().unwrap() {
        assert_eq!(
            digest(&std::fs::read(root.join(name)).unwrap()),
            sha.as_str().unwrap(),
            "{name}"
        );
    }
    transport
}
fn tree_digest(root: &Path) -> String {
    fn scan(root: &Path, path: &Path, files: &mut BTreeMap<String, String>) {
        for entry in std::fs::read_dir(path).unwrap() {
            let e = entry.unwrap();
            assert!(!e.file_type().unwrap().is_symlink());
            if e.file_type().unwrap().is_dir() {
                scan(root, &e.path(), files)
            } else {
                files.insert(
                    e.path()
                        .strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .into(),
                    digest(&std::fs::read(e.path()).unwrap()),
                );
            }
        }
    }
    let mut files = BTreeMap::new();
    scan(root, root, &mut files);
    digest(&serde_json::to_vec(&files).unwrap())
}
#[derive(Clone)]
struct FixtureAuthority {
    producers: BTreeMap<String, ProducerRecord>,
    approvals: BTreeMap<String, ApprovalRecord>,
}
impl AuthorityProvider for FixtureAuthority {
    fn verify_producer(
        &self,
        _: &GuardRunEnvelope,
        digest: &str,
    ) -> Result<ProducerRecord, AuthorityError> {
        self.producers
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
fn validity(v: &Value) -> Validity {
    Validity {
        issued_at: v["issued_at"].as_i64().unwrap(),
        expires_at: v["expires_at"].as_i64().unwrap(),
        revoked: v["revoked"].as_bool().unwrap(),
    }
}
fn policy(v: &Value) -> EligibilityPolicy {
    EligibilityPolicy {
        binding: serde_json::from_value(v["binding"].clone()).unwrap(),
        producer: serde_json::from_value(v["producer"].clone()).unwrap(),
        required_scopes: serde_json::from_value(v["required_scopes"].clone()).unwrap(),
        contract_digest: v["contract_digest"].as_str().unwrap().into(),
        action: v["action"].as_str().unwrap().into(),
        producer_principals: serde_json::from_value(v["producer_principals"].clone()).unwrap(),
        approval_principals: serde_json::from_value(v["approval_principals"].clone()).unwrap(),
    }
}
struct Group {
    root: PathBuf,
    repo: Repository,
    candidate: CandidateSnapshot,
    policies: BTreeMap<String, EligibilityPolicy>,
    envelopes: BTreeMap<String, GuardRunEnvelope>,
    bytes: ImmutableArtifactBytes,
    authority: FixtureAuthority,
}
impl Group {
    fn load(root: PathBuf, repo: Repository, name: &str) -> Self {
        let candidate: CandidateSnapshot =
            serde_json::from_value(value(root.join("gitguard/candidate.json"))).unwrap();
        candidate.validate(&repo).unwrap();
        let rawpolicies = value(root.join("policies.json"));
        let scopes = value(root.join("scopes.json"));
        let mut policies = BTreeMap::new();
        let mut envelopes = BTreeMap::new();
        let mut artifacts = vec![];
        for domain in DOMAINS {
            let envelope: GuardRunEnvelope =
                serde_json::from_value(value(root.join(domain).join("envelope.json"))).unwrap();
            policies.insert(
                domain.into(),
                policy(&rawpolicies[scopes[domain].as_str().unwrap()]),
            );
            for (file, reference) in [
                ("contract", &envelope.artifacts.contract),
                ("facts", &envelope.artifacts.facts),
                ("report", &envelope.artifacts.report),
            ] {
                artifacts.push((
                    reference.clone().unwrap(),
                    std::fs::read(root.join(domain).join(format!("{file}.json"))).unwrap(),
                ));
            }
            envelopes.insert(domain.into(), envelope);
        }
        let captured = value(
            root.parent()
                .unwrap()
                .join(format!("authority-{name}.json")),
        );
        let mut authority = FixtureAuthority {
            producers: BTreeMap::new(),
            approvals: BTreeMap::new(),
        };
        for p in captured["producers"].as_array().unwrap() {
            let d = p["envelope_digest"].as_str().unwrap().to_string();
            authority.producers.insert(
                d.clone(),
                ProducerRecord {
                    principal: p["principal"].as_str().unwrap().into(),
                    producer: serde_json::from_value(p["producer"].clone()).unwrap(),
                    envelope_digest: d,
                    validity: validity(p),
                },
            );
        }
        for p in captured["approvals"].as_array().unwrap() {
            authority.approvals.insert(
                p["reference"].as_str().unwrap().into(),
                ApprovalRecord {
                    principal: p["principal"].as_str().unwrap().into(),
                    purpose: p["purpose"].as_str().unwrap().into(),
                    action: p["action"].as_str().unwrap().into(),
                    binding: serde_json::from_value(p["binding"].clone()).unwrap(),
                    contract_digest: p["contract_digest"].as_str().unwrap().into(),
                    validity: validity(p),
                },
            );
        }
        // The original FG fixture authority carries review records, not GG's new baseline port.
        // Add explicitly NEW fixture records, never relabel the original SG baseline or envelope.
        for (domain, p) in &policies {
            if p.binding.baseline_digest.is_some() {
                authority.approvals.insert(
                    format!("fixture:gg-baseline-{domain}"),
                    ApprovalRecord {
                        principal: "fixture-baseline-owner".into(),
                        purpose: "specification-baseline".into(),
                        action: p.action.clone(),
                        binding: p.binding.clone(),
                        contract_digest: p.contract_digest.clone(),
                        validity: Validity {
                            issued_at: NOW - 10,
                            expires_at: NOW + 100,
                            revoked: false,
                        },
                    },
                );
            }
        }
        Self {
            root,
            repo,
            candidate,
            policies,
            envelopes,
            bytes: ImmutableArtifactBytes::new(artifacts).unwrap(),
            authority,
        }
    }
    fn prepare(&self, drift: Option<&str>) -> PreparedSpecialistConsumption {
        let mut required = vec![];
        for (domain, original) in &self.policies {
            let mut p = original.clone();
            if domain == "specguard" {
                match drift {
                    Some("baseline") => {
                        p.binding.baseline_digest = Some(format!("sha256:{}", "b".repeat(64)))
                    }
                    Some("source") => {
                        p.binding.source_snapshot_digest = format!("sha256:{}", "c".repeat(64))
                    }
                    Some("rules") => p.contract_digest = format!("sha256:{}", "d".repeat(64)),
                    Some("analyzer") => {
                        p.producer.analyzer_version = "next-controller-selected-version".into()
                    }
                    _ => {}
                }
            }
            let baseline = p.binding.baseline_digest.as_ref().map(|_| {
                let mut policy = p.clone();
                policy.approval_principals = BTreeMap::from([(
                    "specification-baseline".into(),
                    ["fixture-baseline-owner".into()].into(),
                )]);
                BaselineApproval {
                    reference: format!("fixture:gg-baseline-{domain}"),
                    policy,
                    purpose: "specification-baseline".into(),
                }
            });
            required.push(RequiredSpecialist {
                id: domain.clone(),
                policy: p,
                profile: EvidenceProfile::EngineBacked,
                baseline,
            });
        }
        PreparedSpecialistConsumption::freeze(&self.repo, &self.candidate, required).unwrap()
    }
    fn consume(
        &self,
        prepared: &PreparedSpecialistConsumption,
        authority: &FixtureAuthority,
        now: i64,
    ) -> SpecialistConsumption {
        prepared
            .consume(
                &self.repo,
                &self.candidate,
                &self.envelopes,
                &self.bytes,
                authority,
                now,
                None,
            )
            .unwrap()
    }
    fn gg_bundle(&self) -> CheckBundle {
        serde_json::from_value(value(self.root.join("gitguard/bundle.json"))).unwrap()
    }
}
fn outcome(result: &SpecialistConsumption) -> Value {
    json!({"eligible":result.eligible(),"domains":result.results().iter().map(|(name,r)|(name,json!({"code":r.code,"technical_decision":r.technical_decision}))).collect::<BTreeMap<_,_>>()})
}
fn late_completion(m1: &Group, m2: &Group) -> bool {
    let store = LocalHistory::new(StorageRequirement::ProcessLocal).unwrap();
    let key = |g: &Group| {
        ReuseKey::new(
            &g.repo,
            &g.candidate,
            &g.policies["gitguard"],
            &format!("sha256:{}", "a".repeat(64)),
            &format!("sha256:{}", "b".repeat(64)),
        )
        .unwrap()
    };
    let k1 = key(m1);
    let k2 = key(m2);
    let b1 = m1.gg_bundle();
    let b2 = m2.gg_bundle();
    let t1 = store.begin(&k1, 0, &b1.envelope.run_id).unwrap();
    let t2 = store.begin(&k2, 1, &b2.envelope.run_id).unwrap();
    let c2 = consume_bound(
        &m2.repo,
        &m2.candidate,
        &b2,
        &m2.policies["gitguard"],
        &m2.authority,
        ConsumptionContext {
            key: &k2,
            now: NOW,
            cause: None,
        },
    )
    .unwrap();
    assert!(c2.result().eligible);
    store.complete(&t2, &c2).unwrap();
    store.publish(&t2).unwrap();
    let c1 = consume_bound(
        &m1.repo,
        &m1.candidate,
        &b1,
        &m1.policies["gitguard"],
        &m1.authority,
        ConsumptionContext {
            key: &k1,
            now: NOW,
            cause: None,
        },
    )
    .unwrap();
    assert!(c1.result().eligible);
    store.complete(&t1, &c1).unwrap();
    assert!(store.publish(&t1).is_err());
    let target = guardengine::integration::attempt_store::Target {
        repo_id: m2.candidate.repo_id().into(),
        task_id: m2.candidate.task_id().into(),
        requirement_ids: m2.candidate.requirement_ids().to_vec(),
    };
    assert_eq!(
        store.current(&target).unwrap().unwrap().run_id,
        b2.envelope.run_id
    );
    assert_eq!(store.history(&target).unwrap().len(), 2);
    true
}
/// The same reviewed M2 corpus and local writer, without replaying the four-group matrix.
/// Kept separate from the older Gitflow candidate/profile used by local_rollout.
pub fn required_rollout() -> Result<Value, String> {
    if !cfg!(feature = "privileged-execution") {
        return Err("required rollout fixture requires privileged-execution".into());
    }
    let corpus = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/end-to-end/joint"
    ));
    verify_corpus(corpus);
    let fixture = tempfile::Builder::new()
        .prefix(".required-rollout-")
        .tempdir_in(std::env::current_dir().map_err(|_| "working directory unavailable")?)
        .map_err(|_| "fixture unavailable")?;
    let repository = fixture.path().join("repository");
    git(
        fixture.path(),
        &[
            "clone",
            "-q",
            corpus.join("source.bundle").to_str().unwrap(),
            repository.to_str().unwrap(),
        ],
    );
    let manifest = value(corpus.join("producer-manifest.json"));
    let worktree = fixture.path().join("worktree-M2");
    git(
        &repository,
        &[
            "worktree",
            "add",
            "--quiet",
            "--detach",
            worktree.to_str().unwrap(),
            manifest["profiles"]["M2"]["candidate"].as_str().unwrap(),
        ],
    );
    let repo = Repository::discover(&worktree, "cross-guard-repo").unwrap();
    let group = Group::load(corpus.join("M2"), repo, "M2");
    let before = tree_digest(&repository);
    let worktree_before = tree_digest(&worktree);
    let mut result = local_execution(fixture.path(), &repository, &group);
    assert_eq!(before, tree_digest(&repository));
    assert_eq!(worktree_before, tree_digest(&worktree));
    verify_corpus(corpus);
    result["profile"] = json!("gitguard.required-specialist-local-rollout/M2");
    result["transport_sha256"] = json!(TRANSPORT_SHA);
    result["source_unchanged"] = json!(true);
    result["original_artifacts_unchanged"] = json!(true);
    result["eligibility_time_boundary"] = json!(
        "fresh consumption before each apply; not atomic future authority across external providers"
    );
    Ok(result)
}

pub fn run() -> Result<Value, String> {
    if !cfg!(feature = "privileged-execution") {
        return Err("joint mutation fixture requires explicit privileged-execution build".into());
    }
    let corpus = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/end-to-end/joint"
    ));
    let transport = verify_corpus(corpus);
    let fixture = tempfile::Builder::new()
        .prefix(".joint-consumption-")
        .tempdir_in(std::env::current_dir().unwrap())
        .unwrap();
    let repository = fixture.path().join("repository");
    git(
        fixture.path(),
        &[
            "clone",
            "-q",
            corpus.join("source.bundle").to_str().unwrap(),
            repository.to_str().unwrap(),
        ],
    );
    let manifest = value(corpus.join("producer-manifest.json"));
    let mut groups = BTreeMap::new();
    for name in ["A", "B", "M1", "M2"] {
        let worktree = fixture.path().join(format!("worktree-{name}"));
        git(
            &repository,
            &[
                "worktree",
                "add",
                "--quiet",
                "--detach",
                worktree.to_str().unwrap(),
                manifest["profiles"][name]["candidate"].as_str().unwrap(),
            ],
        );
        let repo = Repository::discover(&worktree, "cross-guard-repo").unwrap();
        groups.insert(name, Group::load(corpus.join(name), repo, name));
    }
    let baseline_a = value(corpus.join("A/specguard/baseline.json"));
    let baseline_b = value(corpus.join("B/specguard/baseline.json"));
    assert_eq!(baseline_a["sourceDigest"], baseline_b["sourceDigest"]);
    assert_eq!(baseline_a["graphDigest"], baseline_b["graphDigest"]);
    assert_eq!(baseline_a["sourceRevision"], baseline_b["sourceRevision"]);
    assert_ne!(baseline_a["scope"], baseline_b["scope"]);
    let before = tree_digest(&repository);
    let worktree_before: BTreeMap<_, _> = ["A", "B", "M1", "M2"]
        .into_iter()
        .map(|n| {
            (
                n,
                tree_digest(&fixture.path().join(format!("worktree-{n}"))),
            )
        })
        .collect();
    let mut observations = BTreeMap::new();
    // A/B run concurrently with distinct owned frozen contexts and physical worktrees.
    std::thread::scope(|scope| {
        let handles: [_; 2] = ["A", "B"].map(|name| {
            let g = &groups[name];
            scope.spawn(move || {
                let prepared = g.prepare(None);
                assert!(g.consume(&prepared, &g.authority, NOW).eligible());
            })
        });
        for h in handles {
            h.join().unwrap();
        }
    });
    for (name, g) in &groups {
        let prepared = g.prepare(None);
        let result = g.consume(&prepared, &g.authority, NOW);
        assert!(
            result.eligible(),
            "{name}: {:?}",
            result
                .results()
                .iter()
                .map(|(k, v)| (k, v.code))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            result.results()["specguard"].technical_decision,
            Some(Decision::RequireApproval)
        );
        let fg = value(corpus.join(format!("{name}.stdout.json")));
        assert_eq!(fg["execution_authorized"], false);
        assert_eq!(fg["envelope"]["decision"], "ALLOW");
        assert!(execution::request(&ExecutionConfig { enabled: true }, Operation::Push).is_err());
        let mut negative = BTreeMap::new();
        let expired = g.consume(&prepared, &g.authority, NOW + 100);
        assert!(!expired.eligible());
        negative.insert("expiry", outcome(&expired));
        let mut revoked = g.authority.clone();
        revoked
            .approvals
            .get_mut("fixture:gg-baseline-specguard")
            .unwrap()
            .validity
            .revoked = true;
        let baseline_revoked = g.consume(&prepared, &revoked, NOW);
        assert!(!baseline_revoked.eligible());
        negative.insert("baseline_revocation", outcome(&baseline_revoked));
        let mut review_revoked = g.authority.clone();
        review_revoked
            .approvals
            .get_mut("fixture:actual-spec-review")
            .unwrap()
            .validity
            .revoked = true;
        let rejected = g.consume(&prepared, &review_revoked, NOW);
        assert!(!rejected.eligible());
        negative.insert("review_revocation", outcome(&rejected));
        assert_eq!(
            rejected.results()["specguard"].technical_decision,
            Some(Decision::RequireApproval)
        );
        let mut missing = g.envelopes.clone();
        missing.remove("archguard");
        let missing_error = prepared
            .consume(
                &g.repo,
                &g.candidate,
                &missing,
                &g.bytes,
                &g.authority,
                NOW,
                None,
            )
            .err()
            .expect("missing domain must fail");
        negative.insert("missing_domain", json!({"error":missing_error}));
        for drift in ["baseline", "source", "rules", "analyzer"] {
            let rejected = g.consume(&g.prepare(Some(drift)), &g.authority, NOW);
            assert!(!rejected.eligible(), "{name}:{drift}");
            negative.insert(drift, outcome(&rejected));
        }
        observations.insert(*name,json!({"candidate":g.candidate.candidate_oid(),"base":g.candidate.base_oid(),"task":g.candidate.task_id(),"requirements":g.candidate.requirement_ids(),"logical_worktree":g.candidate.worktree_id(),"group":g.candidate.merge_group_id(),"baseline_digest":g.policies["specguard"].binding.baseline_digest,"eligible":true,"technical_specguard":"REQUIRE_APPROVAL","fg_allow_grants_execution":false,"negative_outcomes":negative,"specialist_audits":result.results().iter().map(|(k,r)|(k,&r.audit)).collect::<BTreeMap<_,_>>() }));
    }
    for (old, new) in [("A", "B"), ("A", "M1"), ("M1", "M2")] {
        let g = &groups[new];
        let old = &groups[old];
        let result = g
            .prepare(None)
            .consume(
                &g.repo,
                &g.candidate,
                &old.envelopes,
                &old.bytes,
                &old.authority,
                NOW,
                None,
            )
            .unwrap();
        assert!(!result.eligible());
    }
    let late = late_completion(&groups["M1"], &groups["M2"]);
    let execution = local_execution(fixture.path(), &repository, &groups["M2"]);
    assert_eq!(before, tree_digest(&repository));
    for (name, expected) in &worktree_before {
        assert_eq!(
            *expected,
            tree_digest(&fixture.path().join(format!("worktree-{name}")))
        );
    }
    verify_corpus(corpus);
    Ok(
        json!({"profile":"gitguard.four-group-local-consumption/v1","production_authenticated":false,"producer_rerun":false,"transport_sha256":TRANSPORT_SHA,"shared_project_digest":manifest["project_digest"],"shared_baseline_graph_digest":baseline_a["graphDigest"],"retained_source_pins":transport["source_pins"],"groups":observations,"original_artifacts_unchanged":true,"source_unchanged":true,"source_before":before,"source_after":tree_digest(&repository),"physical_worktree_digests":worktree_before,"cross_context_rejected":["A-to-B","A-to-M1","M1-to-M2"],"m1_late_publication_refused":late,"late_history_profile":"existing GG CheckBundle LocalHistory, not specialist receipt persistence","new_baseline_authority_profile":"new explicit fixture records, not authentication of retained captures","execution":execution}),
    )
}
#[allow(dead_code)]
fn main() {
    match run() {
        Ok(report) => println!("{}", serde_json::to_string_pretty(&report).unwrap()),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(4);
        }
    }
}

fn local_execution(fixture: &Path, source: &Path, group: &Group) -> Value {
    local_write::run(fixture, source, group)
}
mod local_write {
    use super::*;
    use gitguard::execution::{
        apply::{ApplyContext, ApplyError, LocalCasReceipt, apply},
        grant::*,
        intent::IntentStore,
        platform_write::ProtectedBareTarget,
        reconcile::{RecoveryState, reconcile},
    };
    use std::{
        collections::BTreeSet,
        os::unix::fs::PermissionsExt,
        sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    };
    struct Clock;
    impl GrantClock for Clock {
        fn adapter_id(&self) -> &str {
            "fixture-clock"
        }
        fn now(&self) -> Result<i64, PortError> {
            Ok(20)
        }
    }
    struct Authority {
        record: OperationGrant,
        calls: AtomicUsize,
        revoke_after_claim: bool,
    }
    impl GrantAuthority for Authority {
        fn adapter_id(&self) -> &str {
            "fixture-authority"
        }
        fn authenticate(&self, reference: &str) -> Result<OperationGrant, PortError> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            if reference != "fixture:grant" {
                return Err(PortError::Untrusted);
            }
            let mut grant = self.record.clone();
            if self.revoke_after_claim && call > 0 {
                grant.revoked = true;
            }
            Ok(grant)
        }
    }
    fn session(
        repo: &Repository,
        candidate: &CandidateSnapshot,
        operation: &str,
        branch: &str,
        revoke: bool,
    ) -> (GrantSession, Authority) {
        let request = GrantRequest {
            actor: "fixture".into(),
            action: GrantAction::Push,
            target: format!("refs/heads/{branch}"),
            expected_target_oid: Some(candidate.base_oid().into()),
            operation_id: operation.into(),
        };
        let authority = Authority {
            record: OperationGrant {
                api_version: "gitguard.operation-grant/v1alpha1".into(),
                issuer: "fixture".into(),
                actor: request.actor.clone(),
                action: request.action,
                repo_id: candidate.repo_id().into(),
                candidate_oid: candidate.candidate_oid().into(),
                binding_digest: candidate.binding_digest(),
                target: request.target.clone(),
                expected_target_oid: request.expected_target_oid.clone(),
                operation_id: operation.into(),
                not_before: 0,
                expires_at: 100,
                revoked: false,
            },
            calls: AtomicUsize::new(0),
            revoke_after_claim: revoke,
        };
        let expected = GrantExpectation::freeze(
            repo,
            candidate,
            request,
            GrantPolicy {
                authority_adapter: "fixture-authority".into(),
                clock_adapter: "fixture-clock".into(),
                allowed_issuers: BTreeSet::from(["fixture".into()]),
                max_lifetime_seconds: 120,
            },
        )
        .unwrap();
        (GrantSession::new(expected), authority)
    }

    #[derive(Debug, PartialEq, Eq)]
    enum DispatchError {
        RequiredSpecialists,
        Apply(ApplyError),
    }
    // Private finite controller composition, not a transferable authorization token.
    // Each real dispatch rechecks the frozen full required set before entering grant/apply.
    fn checked_apply(
        context: &ApplyContext<'_>,
        target: &ProtectedBareTarget,
        group: &Group,
        required: &PreparedSpecialistConsumption,
        envelopes: &BTreeMap<String, GuardRunEnvelope>,
        provider: &dyn AuthorityProvider,
    ) -> Result<LocalCasReceipt, DispatchError> {
        let assessment = required
            .consume(
                context.repository,
                context.candidate,
                envelopes,
                &group.bytes,
                provider,
                NOW,
                None,
            )
            .map_err(|_| DispatchError::RequiredSpecialists)?;
        if !assessment.eligible() {
            return Err(DispatchError::RequiredSpecialists);
        }
        apply(context, target).map_err(DispatchError::Apply)
    }

    pub(super) fn run(fixture: &Path, source: &Path, group: &Group) -> Value {
        let required = group.prepare(None);
        let candidate = &group.candidate;
        let repo = &group.repo;
        let bare = fixture.join("local-target.git");
        std::fs::create_dir(&bare).unwrap();
        git(&bare, &["init", "--bare", "-q"]);
        fn copy_objects(from: &Path, to: &Path) {
            std::fs::create_dir_all(to).unwrap();
            for entry in std::fs::read_dir(from).unwrap() {
                let entry = entry.unwrap();
                assert!(!entry.file_type().unwrap().is_symlink());
                if entry.file_type().unwrap().is_dir() {
                    copy_objects(&entry.path(), &to.join(entry.file_name()));
                } else {
                    std::fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
                }
            }
        }
        copy_objects(&source.join(".git/objects"), &bare.join("objects"));
        std::fs::set_permissions(&bare, std::fs::Permissions::from_mode(0o700)).unwrap();
        for branch in ["checked", "unknown"] {
            git(
                &bare,
                &[
                    "update-ref",
                    &format!("refs/heads/{branch}"),
                    candidate.base_oid(),
                ],
            );
        }
        let target = ProtectedBareTarget::open(&bare, candidate.repo_id()).unwrap();
        let intent_root = fixture.join("intents");
        std::fs::create_dir(&intent_root).unwrap();
        std::fs::set_permissions(&intent_root, std::fs::Permissions::from_mode(0o700)).unwrap();
        let intents = IntentStore::create(&intent_root).unwrap();
        let (publish, authority) = session(repo, candidate, "joint-M2-publish", "checked", false);
        let config = ExecutionConfig { enabled: true };
        let cancel = AtomicBool::new(false);
        let mut context = ApplyContext {
            config: &config,
            repository: repo,
            candidate,
            session: &publish,
            authority: &authority,
            clock: &Clock,
            grant_reference: "fixture:grant",
            intents: &intents,
            cancel: &cancel,
        };
        // All negative checks hit this actual dispatch before a grant call, intent or ref write.
        let initial_history = std::fs::read(intent_root.join("intent.json")).unwrap();
        let initial_target = tree_digest(&bare);
        let mut refusals = BTreeMap::new();
        for case in [
            "missing_required",
            "producer_revoked",
            "producer_expired",
            "baseline_revoked",
            "adapter_rollback",
        ] {
            let mut provider = group.authority.clone();
            let mut envelopes = group.envelopes.clone();
            match case {
                "missing_required" => {
                    envelopes.remove("testguard");
                }
                "producer_revoked" => {
                    provider
                        .producers
                        .values_mut()
                        .next()
                        .unwrap()
                        .validity
                        .revoked = true
                }
                "producer_expired" => {
                    provider
                        .producers
                        .values_mut()
                        .next()
                        .unwrap()
                        .validity
                        .expires_at = NOW
                }
                "baseline_revoked" => {
                    provider
                        .approvals
                        .get_mut("fixture:gg-baseline-specguard")
                        .unwrap()
                        .validity
                        .revoked = true
                }
                "adapter_rollback" => {
                    envelopes.remove("codeguard");
                }
                _ => unreachable!(),
            }
            assert!(
                matches!(
                    checked_apply(&context, &target, group, &required, &envelopes, &provider),
                    Err(DispatchError::RequiredSpecialists)
                ),
                "{case}"
            );
            assert_eq!(authority.calls.load(Ordering::SeqCst), 0);
            assert_eq!(initial_target, tree_digest(&bare));
            assert_eq!(
                initial_history,
                std::fs::read(intent_root.join("intent.json")).unwrap()
            );
            refusals.insert(
                case,
                json!({"outcome":"RequiredSpecialists", "grant_calls":0,
                "intent_unchanged":true, "target_unchanged":true, "required_count":DOMAINS.len()}),
            );
        }
        // A valid target and frozen request still require a separately authenticated operation grant.
        context.grant_reference = "fixture:FG-ALLOW-is-not-a-grant";
        let before = tree_digest(&bare);
        assert!(matches!(
            checked_apply(
                &context,
                &target,
                group,
                &required,
                &group.envelopes,
                &group.authority
            ),
            Err(DispatchError::Apply(ApplyError::Authority))
        ));
        assert_eq!(before, tree_digest(&bare));
        context.grant_reference = "fixture:grant";
        cancel.store(true, Ordering::SeqCst);
        assert!(matches!(
            checked_apply(
                &context,
                &target,
                group,
                &required,
                &group.envelopes,
                &group.authority
            ),
            Err(DispatchError::Apply(ApplyError::Cancelled))
        ));
        assert_eq!(before, tree_digest(&bare));
        assert_eq!(
            reconcile(&publish, &intents, &target).unwrap(),
            RecoveryState::NotRecorded
        );
        // Explicit fixture authorization is separate from all eligibility/gate results.
        cancel.store(false, Ordering::SeqCst);
        let receipt = checked_apply(
            &context,
            &target,
            group,
            &required,
            &group.envelopes,
            &group.authority,
        )
        .unwrap();
        assert_eq!(
            git(&bare, &["rev-parse", "refs/heads/checked"]),
            candidate.candidate_oid()
        );
        let (uncertain, revoked) = session(repo, candidate, "joint-M2-unknown", "unknown", true);
        context.session = &uncertain;
        context.authority = &revoked;
        assert!(matches!(
            checked_apply(
                &context,
                &target,
                group,
                &required,
                &group.envelopes,
                &group.authority
            ),
            Err(DispatchError::Apply(ApplyError::RecoveryRequired))
        ));
        assert_eq!(
            git(&bare, &["rev-parse", "refs/heads/unknown"]),
            candidate.base_oid()
        );
        let preserved = std::fs::read(intent_root.join("intent.json")).unwrap();
        let disabled = ExecutionConfig::default();
        context.config = &disabled;
        assert!(matches!(
            checked_apply(
                &context,
                &target,
                group,
                &required,
                &group.envelopes,
                &group.authority
            ),
            Err(DispatchError::Apply(ApplyError::Disabled))
        ));
        assert!(group.consume(&required, &group.authority, NOW).eligible());
        drop(intents);
        let reopened = IntentStore::open(&intent_root).unwrap();
        assert!(matches!(
            reconcile(&publish, &reopened, &target).unwrap(),
            RecoveryState::Applied { .. }
        ));
        assert_eq!(
            reconcile(&uncertain, &reopened, &target).unwrap(),
            RecoveryState::RecoveryRequired
        );
        assert_eq!(
            preserved,
            std::fs::read(intent_root.join("intent.json")).unwrap()
        );
        json!({"required_count":DOMAINS.len(),"required_scopes":group.policies.iter().map(|(id,p)|(id,&p.required_scopes)).collect::<BTreeMap<_,_>>(),"rejected_before_grant":refusals.len(),"required_check_refusals":refusals,"adapter_rollback_required_set_preserved":true,"technical_specguard":group.envelopes["specguard"].decision,"candidate":candidate.candidate_oid(),"target_before":candidate.base_oid(),"fg_allow_and_consumption_without_operation_grant":"Authority error; target bytes unchanged","cancelled_ref_unchanged":true,"cancelled_operation":"NotRecorded","explicit_fixture_grant_only_write":true,"operation_id":receipt.operation_id(),"request_digest":receipt.request_digest(),"actual_target":git(&bare,&["rev-parse","refs/heads/checked"]),"executor_rollback":"Disabled; read-only consumption still eligible","reopened_applied":true,"reopened_unknown":"RecoveryRequired","history_unchanged":true,"production_authority":false})
    }
}
