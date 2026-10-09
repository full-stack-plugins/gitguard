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


## Independent-review P2 fix — scope binding

Review: /workspace/guard-implementation-ledger/gitguard-review.md. Confirmed actual allowed path prefixes were omitted from snapshot identity, so identical caller policy digests concealed different enforced scopes. No task checkbox changed.

Ruling: bind the actual canonical `allowed_paths` directly, alongside the existing task, requirements, policy and baseline fields. Normalize only lexicographic unsigned-byte sorting and exact duplicate removal at snapshot preparation. Retain byte/case distinctions and redundant ancestor/descendant prefixes; do not infer normalization from an external policy digest. Deserialized snapshots must already be canonical and safe.

Compatibility: advance local snapshot wire version from gitguard.candidate/v1alpha1 to v1alpha2; reject old records, require allowed_paths, update the real-object vector and schema. All prior binding digests require regeneration. FlowGuard implementer notified; Rust constructor compatibility retained, allowed_paths getter added. No authentication/trust capability changed.

TDD: `cargo test --test scope_binding` RED 0/2: opposite local completeness outcomes had identical digest; allowed_paths was absent. Same command GREEN 2/2 after fix. Logs: gitguard-review-fix-red.log and gitguard-review-fix-green.log. Tests additionally cover byte sorting/non-UTF-8, exact duplicate normalization, unsafe/unsorted/duplicate serialized paths, required field and old-version rejection.

Review-fix verification: full `cargo test` passed 23 integration tests, zero failures; `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `git diff --check` passed. Evidence: gitguard-review-fix-tests.log and gitguard-review-fix-clippy.log. Awaiting root's independent recheck; earlier trust, baseline, filesystem and process-containment limitations remain unchanged.

## Independent review acceptance

Local full task acceptance: 1.3, 1.5, 1.6, 2.1, 2.2, 2.3, 2.4, 2.5, 2.6. Review findings were fixed with RED/GREEN regressions and independently rechecked; see cloud execution ledger gitguard-review.md. Scope remains local and advisory/fixture-labelled where stated. Production authority, remaining capability gaps and hosted gates are not claimed.

## Slice 2 plan — 3.1/3.2/4.1

Base 1d72ba1 (root recorded bounded local task acceptance). Preserve candidate v1alpha2 and FlowGuard consumers.
- 3.1: actual GE GuardFacts/forbid_relation mapping, pinned local mapping version, policy digest matched to actual engine contract; real clean/violation/partial and enforce/review/advise tests. Domain data stays outside engine objects.
- 3.2: strict versioned JSON CLI input and stdout bundle (envelope + exact artifacts); no --report support. Real GE attempt lifecycle when available, no pre-binding envelope, bound errors/cancelled null, decision/exit correspondence and strict input rejection. No stale output file can become success.
- 4.1: default-disabled optional writer configuration and always-denied local port until reviewed external authorization exists. No credentials, transport, grants, remote writes or approval booleans as authority.
Ruling: use sibling GuardEngine source dependency only for local development, not claim published independent distribution. stdout-only CLI deliberately rejects --report; callers must check process code and bundle integrity. Gitflow provider survey deferred from this bounded slice unless spare independent time remains.

Slice 2 implemented pending review:
- 3.1 actual GuardEngine mapping and recomputation; engine policy digest checked against scope; strict relation/version/field checks; partial scope yields real BLOCK/INDETERMINATE. Domain schema emitted separately; no current wire extension.
- 3.2 actual strict check CLI and real GE prepare_attempt/finish lifecycle. stdout-only artifact bundle; no --report. Real 0/2/3/4, post-bind cancellation/error null, immutable required scope set includes candidate binding digest; failed final observations clear coverage. Actual external signal delivery remains untested; cooperative bound token path is covered.
- 4.1 deliberate always-denied optional writer, default feature/config disabled and enabled-feature path still rejects without external authorization. No transport or credentials implemented.
- 3.6 provider survey intentionally not started, keeping this slice small for independent review. GE-TRUST/provider and privileged operations remain separate future work.

TDD: projection unsupported scaffold RED 0/2 -> GREEN2/2; CLI scaffold RED3 failed/1 passed -> GREEN4/4; executor accepting scaffold RED0/2 -> GREEN2/2 in both default and feature-enabled builds; oversized 129-rule contract RED accepted -> rejected GREEN; final error coverage RED retained Complete -> cleared Partial GREEN. Logs under /workspace/guard-implementation-ledger/gitguard-slice2-*.log. Engine's AttemptOutput.coverage was added during this slice; initial executor test compile failure is recorded separately from the subsequent genuine behavioral RED.

Final verification: `cargo test` 32 integration tests passed, zero failures (empty unit/bin/doc suites); feature-enabled executor tests 2/2; `cargo clippy --all-targets --all-features -- -D warnings`, `cargo fmt --check`, `git diff --check` passed; strict OpenSpec passed 1/1 with zero issues. Source dependency GuardEngine observed at 1498970670b4b364430b091836c23cb37abc75f8. No new task checkbox updates before root review.

## Slice 2 independent acceptance registration

Root authorized recording complete tasks 3.1, 3.2 and 4.1 after independent review of be31e5d. Review evidence: /workspace/guard-implementation-ledger/gitguard-slice2-review.md — no concrete P1/P2; 32 integration tests and two feature-enabled denial tests independently passed, plus adversarial actual-Git scope substitution and forged ChangeSet probe.

Exactly 12/28 tasks now accepted: 1.3, 1.5, 1.6, 2.1–2.6, 3.1, 3.2, 4.1. Acceptance retains the local advisory / engine integration / denied-executor limits. Previously partial 1.1, 1.2 and 1.4 remain unchecked; no authenticated provider, protected admission, baseline support or operational writer is thereby accepted.

## Slice 3 plan — GE trust ports, local CAS/freshness/audit, compatibility survey

Base e72b130. Root approved process-local Mutex over actual GE InMemoryAttemptStore; no restart/multiprocess/durable transaction claim, and durable capability requests must reject. Plan: real-engine artifact consumption with fresh AuthorityProvider calls; full candidate/policy/producer/coverage/dependency/config reuse keys; two-thread generation CAS and late-completion isolation; separate authenticated-access audit port with hashed records and bounded retention. Production providers remain unavailable. Report 3.4 as localprofile/partial pending precise independent review; never complete 4.4 durable intent from this work.

Slice 3 implemented for independent review, with tasks unchanged at 12/28:
- 3.3 local GitGuard profile: actual GE eligibility/provider ports, strict candidate and artifact binding, no installed authority provider. Cross-specialist profiles and actual approved non-null baseline are deferred.
- 3.4 localprofile/partial: actual GE in-memory store behind Mutex, store-owned tickets, full reuse digest, per-target generation CAS and immutable completion history. Durable requirement rejects; no cross-process/restart/transaction guarantee, no 4.4 implementation.
- 3.5 localprofile/partial: input drift identity and fresh expiry/revocation checks; bounded sanitized audit records with external identity/ACL and expired-row purge ports. No production identity/audit service.
- 3.6 partial survey only: official gitflow-plugin 0.2.0 at 310c43c5e3fe7effa19245dcc4970249e569004c; observed allow0/deny1/unverified3/usage-error2 plus same-candidate GitGuard ALLOW0. No opt-in adapter enabled; internal error4 source-inspected, not exercised. See adapters/gitflow/compatibility.md and observed.json.

ADR: docs/local-evidence-store-adr.md fixes capability limits, authoritative caller inputs, non-transactional audit/history separation and rejection of durable storage. Candidate v1alpha2 unchanged; reuse v1alpha1 newly local.

TDD: missing consumption RED0/2→GREEN2/2; missing CAS/durable check RED0/2→GREEN2/2; incomplete freshness key RED0/1→GREEN; unauthorized audit read RED0/1→GREEN2/2; rehashed unknown domain field RED0/1→GREEN; cross-store ticket publication RED0/1→GREEN3/3, then genuine two-task same-repo case GREEN4/4. Logs gitguard-slice3-{red,green}-*.log in external ledger.

GE producer budget follow-up: shared evaluate_bounded at GuardEngine 11e1d86 replaces native evaluate in producer and removes copied generic expansion formulas; mapped fact input limits remain domain-owned. New evaluator boundary test using old evaluate was behavioral RED0/1 (128 rules ×4096 facts accepted), then GREEN on shared bounded API. This is a refactoring boundary regression, not a claim that the previous whole producer lacked its own cap. Real 700-path candidate/128-rule integration regression confirms Error/null/no report and unchanged source bytes. Both bound observation and evaluation errors clear coverage and retain attempt identity.

Final verification: offline Cargo full suite 43 tests (1 unit +42 integration), zero failures; feature-enabled executor denial2/2; offline all-target/all-feature Clippy -D warnings, fmt check and git diff --check passed. Strict OpenSpec1/1 passed. Official survey rerun passed clean pinned checkout and unchanged fixture byte assertion. Cargo.lock records actual GE runtime tempfile dependency. Source dependency observed at 11e1d86eb353f006844ebb6eefd93f24b98ee481. Detailed handoff: /workspace/guard-implementation-ledger/gitguard-slice3-report.md. No new task acceptance asserted.
