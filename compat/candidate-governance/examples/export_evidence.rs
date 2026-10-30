//! Real temporary Git fixture -> actual producer artifacts, for cross-consumer verification.
use gitguard::{
    candidate::CandidateRequest,
    cli::CheckRequest,
    evidence::{envelope::BoundCheck, projection::FrozenPolicy},
    scope::TaskScope,
};
use guardengine::Enforcement;
use std::{path::Path, process::Command, sync::atomic::AtomicBool};
fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("/usr/bin/git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", root)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
        .env("GIT_COMMITTER_NAME", "Fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
        .env("GIT_AUTHOR_DATE", "2000-01-01T00:00:00Z")
        .env("GIT_COMMITTER_DATE", "2000-01-01T00:00:00Z")
        .output()
        .unwrap();
    assert!(output.status.success(), "fixture git command failed");
    String::from_utf8(output.stdout).unwrap().trim().into()
}
fn main() {
    let output = std::env::args_os()
        .nth(1)
        .expect("usage: export_evidence NEW_DIRECTORY");
    let output = Path::new(&output);
    std::fs::create_dir(output).expect("output must be a new directory");
    let output = std::fs::canonicalize(output).unwrap();
    let source = tempfile::tempdir().unwrap();
    git(source.path(), &["init", "-q", "--object-format=sha1"]);
    std::fs::write(source.path().join("a"), "base\n").unwrap();
    git(source.path(), &["add", "a"]);
    git(source.path(), &["commit", "-qm", "base"]);
    let base = git(source.path(), &["rev-parse", "HEAD"]);
    std::fs::write(source.path().join("a"), "candidate\n").unwrap();
    git(source.path(), &["add", "a"]);
    git(source.path(), &["commit", "-qm", "candidate"]);
    let head = git(source.path(), &["rev-parse", "HEAD"]);
    let policy = FrozenPolicy::new(Enforcement::Enforce);
    let bundle = BoundCheck::prepare(CheckRequest {
        api_version: "gitguard.check/v1alpha1".into(),
        repo_root: source.path().into(),
        repo_id: "golden-repo".into(),
        candidate_oid: head,
        scope: TaskScope::advisory(
            "golden-task",
            vec!["R-GOLDEN".into()],
            vec![b"a".to_vec()],
            &policy.digest(),
            None,
        )
        .unwrap(),
        candidate: CandidateRequest {
            worktree_id: "golden-worktree".into(),
            base_oid: base,
            merge_group_id: None,
            members: vec![],
        },
        contract: policy.contract(),
    })
    .unwrap()
    .run(&AtomicBool::new(false))
    .unwrap();
    assert_eq!(bundle.exit_code(), 0);
    for (name, value) in [
        ("contract.json", bundle.contract.as_ref().unwrap()),
        ("facts.json", bundle.facts.as_ref().unwrap()),
        ("report.json", bundle.report.as_ref().unwrap()),
        ("domain.json", &bundle.domain),
    ] {
        std::fs::write(output.join(name), serde_json::to_vec(value).unwrap()).unwrap();
    }
    std::fs::write(
        output.join("envelope.json"),
        serde_json::to_vec(&bundle.envelope).unwrap(),
    )
    .unwrap();
    std::fs::write(
        output.join("bundle.json"),
        serde_json::to_vec(&bundle).unwrap(),
    )
    .unwrap();
    // Keep the actual source commit/trees/blobs inspectable without touching a user repository.
    git(
        source.path(),
        &[
            "bundle",
            "create",
            output.join("source.bundle").to_str().unwrap(),
            "--all",
        ],
    );
    println!("{}", output.display());
}
