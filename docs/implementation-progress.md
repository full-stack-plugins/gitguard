# Implementation progress — add-candidate-bound-git-governance

Base: 5c511ccbd7c6d11c01dab5d80deff0a4f15383dd. Isolated implementation branch; no baseline runtime/build/tests exist.

Using executing-plans and test-driven-development. Root owns review and checkbox acceptance.

| Tasks | Concrete implementation / tests | Prerequisites |
|---|---|---|
| 1.1–1.2 | Repository discovery, private object mirror, constrained Git runner; real SHA1/SHA256 and malicious config tests | local Git |
| 1.3–1.6 | typed subjects, frozen scope, NUL raw diff and preflight; dirty, rename, binary paths, submodule tests | discovery/runner |
| 2.1–2.2 | temporary candidate construction and exact queue bindings; parent/group/order drift tests | immutable subjects |
| 2.3–2.4 | lease registry and explicit graph coverage; cancellation/reuse and API risk tests | scoped identities |
| 2.5–2.6 | strict candidate serialization/schema and composed read-only acceptance; docs | prior slice |

Pre-flight: 1.1 repository object format supplies 1.3 OID checks; 1.3 frozen subject supplies 2.1 candidate; 1.4 policy is caller-frozen and never loaded from candidate; 2.2 queue authentication remains external and advisory. Shared envelope/engine fields are not invented.

Ruling: Linux/Unix first, pinned system /usr/bin/git; copy bounded object store into private temp metadata and reject alternate/symlink object storage. This avoids reading untrusted repository config during observation. Cost: large or alternate-backed repositories fail closed instead of being supported.
Ruling: full immutable OIDs only; mutable revision names are not accepted. Repo IDs are caller-assigned local labels and never claimed authenticated; controller registration remains later trust integration.
Ruling: no auto-fix, credentials, network, or Git writes to source repositories. Candidate creation writes only private temporary metadata. No task checkboxes change before root review.

## Slice execution / review handoff

Scope pause from root was lifted by a subsequent explicit continue instruction; implementation resumed without commits during the pause. This ledger and the final report supersede the temporary interrupted report.

Implemented pending review: repository/object isolation; subject/index/worktree distinctions; frozen local scope and raw NUL diff; composed advisory preflight; private two-parent merge previews; exact queue candidate/group/member bindings; process-local worktree leases; labelled semantic graph fixture port; strict candidate records and reproducible real-object vector. No task checkbox changed.

TDD cycles (logs in /workspace/guard-implementation-ledger):
1. repository unsupported stub RED 0/2 -> GREEN 2/2 (red1-behavior/green1).
2. subjects RED 0/1 and diff RED 0/2 -> full GREEN 6 tests (red2*/green2).
3. merge construction RED 1/2 and queue RED 0/1 -> full GREEN (red3*/green3).
4. parallel registration RED 0/1 -> full GREEN (red4/green4).
5. composed preflight RED 0/1 and shared API graph RED 0/1 -> full GREEN (red5*/green5).
6. missing nullable baseline and metadata symlink RED; then stale clean flag RED -> full GREEN (red6*/green6).
7. hidden staged changes and deserialized clean flag RED -> full GREEN (red7*/green7).
8. missing nullable merge-group and scope baseline RED -> GREEN (red8*/green8).
9. digest-only baseline accepted RED -> rejected GREEN (red9-baseline/green9-baseline).

Ruling: first local scope profile permits explicit null baseline only, rejecting non-null digest-only claims. Immutable source revision and authenticated baseline policy need a subsequent slice; a baseline-required gate stays unavailable.
Ruling: CandidateRequest in candidate.rs is the resolved queue input port; no redundant adapter forwards an unverified event. Actual provider event authentication remains outside this slice.
Ruling: process-local lease registry only, no durable cross-process cancellation/history guarantee. Cost: callers needing that guarantee cannot use this registry as their scheduler.
Ruling: this slice remains advisory and documents unsupported hostile concurrent directory replacement and OS resource exhaustion rather than claiming a production sandbox. Cost: task 1.2 and protected admission remain partial.

Task status: 1.1 substantial local implementation (MSRV/toolchain matrix pending); 1.2 partial hardening; 1.3 implemented local contract; 1.4 partial (local scope immutable, baseline source/authority unsupported); 1.5 implemented byte-safe static path contract; 1.6 implemented local advisory boundary; 2.1 implemented initial two-parent local preview; 2.2 implemented resolved local exact binding, production event adapter unsupported; 2.3 implemented process-local leases; 2.4 implemented explicitly labelled graph fixture port; 2.5 implemented local null-baseline serialization profile; 2.6 implemented local acceptance, not production authoritative gate. Root decides full task acceptance after review. Groups 3–5 untouched.

Final verification: `cargo fmt --check` passed; `cargo test` passed 21 integration tests, zero failures, plus empty unit/doc suites; `cargo clippy --all-targets -- -D warnings` passed; strict OpenSpec validate passed 1 change with zero issues. Detailed final logs: gitguard-final-tests.log, gitguard-final-clippy.log, gitguard-openspec.log. Initial clippy found one collapsible-if style issue, fixed before the final clean run. Independent review is delegated to root; no full production acceptance asserted.
