# GitGuard engine mapping and CLI v1alpha1

Status: local advisory integration, pending independent slice review. Candidate snapshots remain `gitguard.candidate/v1alpha2`. Engine-backed integrity does not authenticate the repo label, policy, producer, queue event or approver. Baseline remains null-only. No production provider or signer is integrated.

## Projection

`evidence::projection::{FrozenPolicy, project}` owns `gitguard.mapping/v1alpha1`. FrozenPolicy constructs or validates a real native GuardContract; `digest()` is the canonical GuardEngine contract digest without the `sha256:` prefix, matching local TaskScope's 64-hex format. Projection rejects a scope whose supplied policy digest does not match the actual contract. Only exact `forbid_relation { subject: "git-candidate", predicate: "violates", object: "task-scope" }` is supported; all three native enforcement modes are preserved. Unknown operators/fields/version or unsupported relation triples are errors.

A real out-of-scope changed path emits that relation with `source: git-path-sha256:<hash-of-path-bytes>`. No source snippets, credentials or raw path strings enter native facts. Each real violation is visible; ordinary complete scope violations remain complete facts and are evaluated as BLOCK, REQUIRE_APPROVAL or advisory ALLOW by the actual engine. Dirty worktree/index or missing submodule content produces partial facts and a diagnostic, yielding BLOCK/INDETERMINATE regardless of enforcement. Projection revalidates the actual candidate, matches the full v1alpha2 scope binding, and rescans actual Git diff rather than trusting mutable `PreflightResult.complete` or caller-filled ChangeSet fields.

Native facts use `guard.partme.ai/v1alpha1`; subject snapshotDigest is `sha256:<candidate source digest>` and subject ID includes the complete candidate binding digest. Git metadata lives in a separate `gitguard.domain/v1alpha1` artifact with mappingVersion, candidate, actual changes, advisory=true and authorization=not-evaluated. It never extends GuardFacts or GuardReport. Native strict serde loaders reject Git/grant fields in those records.

Limits before evaluation: at most 128 contract rules / 64 KiB serialized contract; at most 4096 facts / 1 MiB fact JSON; bounded rule×fact work and estimated expansion. These are local adapter budgets, not a hostile-process memory sandbox.

## CLI request and output

The sole command is `gitguard check`, reading one JSON request from stdin (maximum 1 MiB). Every other argument, including `--report`, is rejected with exit 4 and empty stdout. No file output or stale-file reuse is implemented. Callers must check exit status as well as parse the complete current stdout.

Request fields (snake_case, strict unknown/duplicate field rejection):
- `api_version`: exactly `gitguard.check/v1alpha1`.
- `repo_root`, `repo_id`, `candidate_oid`: explicit local root/label/full commit OID.
- `scope`: existing TaskScope record; sorted unique requirements, path byte arrays, actual engine policy digest, explicit null baseline.
- `candidate`: existing CandidateRequest (worktree_id/base_oid/merge_group_id/members).
- `contract`: real native GuardContract object, validated by FrozenPolicy.

`cli::CheckRequest` is the typed equivalent. Tests construct requests from real repositories and FrozenPolicy rather than fake OIDs. The library path is `BoundCheck::prepare(request)?.run(&AtomicBool)`; a true atomic cancellation token is not approval or authority.

Successful binding produces a stdout JSON bundle with `api_version`, `envelope`, `contract`, `facts`, `report`, `domain`. Envelope is the actual `guard.integration/v1alpha1` engine-backed profile. Artifacts are embedded JSON values; each `artifact://gitguard/<name>/<digest>` reference resolves to the corresponding embedded field. Digest bytes are compact `serde_json::to_vec(Value)` serialization (object keys sorted lexicographically by the pinned default serde_json map), not the literal whitespace of the outer stdout. JSON contract bytes are also valid YAML accepted by the native loader. Generated completed bundles are checked by `verify_engine_artifacts` before emission; downstream readers should likewise verify the embedded bytes and domain reference, then separately check domain/source binding and external authority if required.

The domain artifact is the frozen local preflight observation. It is not a final technical decision on failed/cancelled attempts. Failed records have no engine report or decision and clear observed coverage; required scopes stay frozen. Required coverage includes `git.scope:<full candidate binding digest>`, `git.source-clean` and `git.submodule-contents`, so group/member/path-scope identity is not lost when projected into the narrower shared RunBinding.

| Outcome | Exit | stdout |
|---|---:|---|
| completed ALLOW | 0 | verified bundle |
| completed BLOCK, including partial | 2 | verified bundle |
| completed REQUIRE_APPROVAL | 3 | verified bundle, unchanged verdict |
| bound observation error/cancellation | 4 | bundle, error/cancelled envelope, null decision/report |
| invalid arguments/input/object/policy or unresolved binding | 4 | empty; generic sanitized stderr |

SIGINT/SIGTERM set a cooperative cancellation token checked at observation boundaries. The API cancellation test exercises cancellation after real binding. Signal interruption before binding, stdout I/O failure or unavailable/invalid clock may prevent a bundle; consumers must treat exit 4 as failure. No claim of instantaneous subprocess cancellation or signal-delivery integration coverage is made. Process-local wall timestamps are observation metadata, never authenticated time. No `approved` input, approval refs, grant issuance, or provider calls exist.

Development uses sibling `../guardengine` with Cargo.lock; independent pinned distribution still waits GE-RELEASE. The mapping test matrix verifies real engine recomputation/enforcement, strict field/version boundaries and partial semantics; CLI tests verify actual subprocess exits and artifacts. Full production trust/containment claims remain blocked by the previous slice's documented limits.
