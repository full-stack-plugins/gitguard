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

## Slice 4 plan — Linux local durable attempt history (3.4)

Base f733989; slice3 independently reviewed elsewhere, no checkbox changes. Add a separate Linux owner-private local-filesystem backend. Stable flock lock, atomic same-directory snapshot replacement with file+directory sync, bounded strict versioned replay records into actual GE InMemoryAttemptStore. Retain existing in-process store and refuse unsupported capabilities. Tests first: reopen preserves immutable history/current and generation, two real processes race with one CAS winner, newer attempt defeats late old publication, corrupt/missing state does not reset, invalid/symlink/permissive directory refuses. Introduce no execution intent, remote write, authority provider or server. Detailed remaining-task dependency inventory: /workspace/guard-implementation-ledger/gitguard-critical-path.md.

Slice3 independent-review P2 corrected first in55718e0: old Consumption could be relabeled under changed dependency/configuration key. `consume_bound` now freezes full ReuseKey before actual provider evaluation; LocalHistory.complete compares private full digest, plain unbound consume cannot complete history. Root probe RED0/1 → race GREEN6/6; full45 passed, Clippy/fmt clean. See external gitguard-review-reuse-fix-report.md. No task accepted by implementer.

Slice4 implemented pending review: separate Linux DurableHistory, strict bounded snapshot replay through actual GE store, private-directory identity/flock, same-directory synced file+atomic rename+directory sync, restart resume and full-key completion. Existing LocalHistory remains process-only; no operation-intent or hosted provider. ADR docs/durable-history-adr.md states precise filesystem/platform/controller assumptions and failure/uncertain outcomes. The environment's /tmp is tmpfs (rejected); tests use owner-private temporary directories on workspace overlayfs. Only overlayfs process crash/restart behavior exercised; hardware power-loss and other filesystem deployments unverified.

TDD: initial memory-only store scaffold RED0/2 for loss on restart and unsafe initialization, actual file backend GREEN. Additional tests cover real two-process CAS, before/after-rename process exits, strict corrupt/invalid-native-transition/unknown-version rejection and genuine bound-consumption restart/relabel denial. A first test run correctly rejected /tmp tmpfs; fixture location moved to supported overlayfs. One helper array-length compile error and one Clippy collapsible-if were corrected; neither counted as behavioral RED.

Final slice4 verification: offline Cargo52 tests passed (7unit including subprocess-worker entry,45integration), zero failures; feature-enabled writer denial2/2; all-target/all-feature offline Clippy -Dwarnings, fmt and diff checks clean; strict OpenSpec1/1. Logs gitguard-slice4-{tests,process-tests,bound-tests,feature,clippy,openspec}.log. Shared GuardEngine c80ec325449842d51007db2fd507040c53dc8f51 now requires yaml-rust2; Cargo.lock reflects actual dependency and is offline validated. No new dependency introduced by local storage. Remaining-task critical path recorded at /workspace/guard-implementation-ledger/gitguard-critical-path.md; 3.4 remains pending independent review and checkboxes stay12/28.

## Independent acceptance — 3.4

Root accepted3.4 after gitguard-reuse-fix-independent-review.md independently reproduced complete reuse binding and cancellation/new-ID retry behavior (45full +7race/probe). Exactly13/28 tasks checked. Acceptance is the process-local ADR profile; df2f756 durable backend remains separately pending review.

## Slice5 plan — complete 1.1 supported local profile and correct current documentation

Inspect1.1/1.2/1.4: linked common-dir and SHA1/SHA256 objects already real-tested; discovery still line-parses config without section/quoting semantics. Complete1.1 by frozen native Git config parsing without includes, strict repository format/version/extension handling, version capability reporting and honest tested toolchain floor. TDD real config with unrelated objectformat, quoted SHA256, unknown format/version/extension, identity independence and source immutability. Current actual executable is /usr/bin/git2.47.3 (prior ADR's2.52.0 was not the pinned runner), Rust1.99.0. Limit supported profile to tested Linux/Git, make untested lower MSRV claim explicit/remove it. 1.2 still lacks full hostile concurrent traversal/resource containment;1.4 needs immutable baseline source and approval port, so neither is claimed complete in this slice. Update bilingual README and technical design to distinguish implemented/reviewed/pending capabilities.

Root reports df2f756 durable backend independently passed52tests+2probes+Clippy with noP1/P2; evidence gitguard-durable-independent-review.md. This strengthens already accepted3.4 and does not complete4.4 or increase13/28.

Slice5 completed for review: task1.1 now has a complete bounded local profile, including native frozen config grammar, section-qualified algorithms, repository version/extension/include rejection, observed actual Git version and caller-label API. No source config becomes operational Git config; actual objects still fsck before use. Real section/quotedSHA256/unknown config/origin identity/source immutability regressions added. Task1.2 containment and1.4 protected baseline remain separately incomplete. No checkbox changed beyond independently authorized3.4 registration (13/28).

TDD `cargo test --offline --test repository`: RED2passed/2failed (unrelated objectformat misinterpreted; unsupported config accepted), then GREEN4/4 plus runner2/2. Official isolated Rust1.90.0 installed after root explicit approval; keep Cargo's declared1.90 floor. Both `cargo +1.90.0 test --locked --offline` and actual1.99 `cargo test --locked --offline` passed54 tests; feature-enabled denial2/2, Clippy all-target/all-feature -Dwarnings, fmt, diff and strict OpenSpec1/1 passed. Actual runner /usr/bin/git2.47.3; earlier2.52.0 ADR claim corrected. Final shared GE6ae9bb4a2f77309dc7bf9216095967c2710c038e.

Bilingual READMEs and technical design now describe implemented/local/reviewed versus future capabilities; architecture's opening likewise distinguishes historical main from this branch. Local links validated. `examples/export_evidence.rs` constructs a real temporary Git repository and uses FrozenPolicy/BoundCheck to emit exact contract/facts/report/envelope/domain/bundle bytes and source.bundle; all artifact hashes checked against envelope. Actual golden directory /workspace/guard-implementation-ledger/gitguard-golden-slice5; no hand-built OIDs/envelope or production authority fixture. Root owns cross-consumer matrix. Detailed handoff gitguard-slice5-report.md in external ledger.

## Reviewed Git configuration and toolchain checkpoint

Task1.1 accepted for the declared local profile after14854ff and independent54-test regression plus two config probes; Rust1.90 focused tests also independently passed. Native Git2.47.3 config parsing is measured, not guessed from workspace Git. Supported SHA1/SHA256/config extension boundary remains explicit. Reviewed total14/28; this does not enable production authority, protected CI or remote writing.

## Git subprocess task1.2 implementation awaiting independent review

Added the explicitly restricted Linux single-process Git profile described in
[git-runner-linux-adr.md](git-runner-linux-adr.md). Fixed executable/argv and private
object-store observation now use audit-architecture-checked seccomp, no_new_privs,
zero core dumps, nonblocking combined-byte capture and bounded direct-child
cleanup. Forbidden process/network syscalls fail explicitly; source config and
inherited helper environments remain isolated. Actual loose/packed Git, candidate,
merge preview, NUL paths and unchanged-source regressions pass. No write executor,
production authority or general sandbox claim is added.

Final full suite68/68, runner-security8/8 and strict all-target Clippy pass. FlowGuard fixed7dbe801 consumer tests20/20
pass against the new runner. TestGuard has no direct GG dependency; new ArchGuard
candidate-source integration still awaits its separate read_commit_files API slice.
Task1.2 is not self-accepted; existing14/28 and capability documentation remain.

## Independently reviewed bounded Git runner: 15/28

Task1.2 is accepted for the explicit Linux x86_64 / trusted `/usr/bin/git` / stable source profile in52f07b08065f2594181102c71866649ee3864cad. Independent review repeated68tests plus3mechanism probes and1real packed/corrupted-object probe; syscall denial, process reaping, pipe budgets and unchanged source state passed. SIGSYS remains ProcessPolicyDenied, never a merge conflict. The reviewer could not exercise a functional compat-int80 environment and makes no compatibility-ABI or aarch64 runtime claim. See cloud ledger `gitguard-runner-independent-review.md`. Subsequent additive commit-tree readerc33b28a is separately pending review.
