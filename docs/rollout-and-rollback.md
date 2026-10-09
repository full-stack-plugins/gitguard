# Local rollout and rollback exercise

Task 5.4 has a runnable local composition, not a hosted enforcement rollout or a new authorization service. Existing production entrypoints and configuration are unchanged. Run:

```sh
cargo run --locked --offline --features privileged-execution --example local_rollout
```

The example accepts no arguments, user repository path, URL or credential. It creates its own private temporary source/bare/intent directories and cleans only that fixture. Without the feature it exits 4 before creating a fixture. The output is JSON with actual candidate/ref OIDs, source/history digests, operation/request identity and phase outcomes.

The source is the pinned Gitflow source bundle already used by the reviewed compatibility adapter. GitGuard runs its current real bounded candidate check and GuardEngine policy evaluation. Gitflow observations are the pinned captured native outputs from that same source/candidate/profile; they are explicitly labelled captured observations, not a new native invocation or a hosted check. The existing adapter verifies their exact object range, protected base policy and schema before comparison. Neither result supplies write authority: the isolated controller still supplies a separate fixture-only GrantAuthority/GrantClock and passes the existing exact candidate/grant/intent/target checks.

The exercise performs these actual stages:

1. Advisory capture: the GitGuard result exists, but disabled execution rejects writes and creates no intent.
2. Shadow comparison: the reviewed `develop` capture and GitGuard ALLOW agree. Comparison alone performs no write.
3. Unresolved difference: the pinned native `main` restriction denies while the GitGuard scope check allows. This demonstration controller has no adjudication for that difference; its write configuration stays disabled and the native target ref stays at the base.
4. Explicit local opt-in: only the active, agreeing, compatible observation is selected for this fixture. The independently authorized Push transfers the exact candidate closure and atomically advances the private bare `develop` ref. A separate claimed operation that loses authorization is recorded uncertain without advancing its ref.
5. Adapter rollback: removing the selected adapter stops scheduling. The resulting route actually rejects a write while the separately owned executor configuration remains unchanged. It does not uninstall or mutate the pinned native provider or its capture.
6. Executor rollback: disabled execution rejects a new valid operation even while read-only shadow comparison remains available. Reopening the intent store still returns Applied for the acknowledged operation and RecoveryRequired for the uncertain one. The journal bytes and source repository bytes remain identical across rollback.

`tests/rollout_rollback.rs` runs the same working composition and asserts the observed results. The example's private comparison/selection functions are demonstration-controller logic; there is no new public permit format, production rollout flag, arbitrary approval Boolean or automatic enforcement activation. Existing callers remain responsible for protecting their controller configuration and selecting an authenticated authority.

Remaining task 5.4 work is specific: integrate the chosen deployment's protected rollout ownership and native differential adjudications, then verify its actual enforcement/release combination. Existing local GuardEngine package verification can be reused; it is not a blocker to this composition. Public distribution and hosted required-check behavior are not established by this example. Task 5.4 stays unchecked until the remaining scope is independently accepted.

Task 3.3 also has a concrete local gap: `CandidateSnapshot::validate` and `TaskScope::validate` currently reject every non-null baseline, while `evidence::consume` validates GitGuard's own CheckBundle rather than resolving policy-required specialist bundles. Adding an authenticated provider alone cannot fix those model/consumer gaps. A follow-up must bind an immutable approved baseline record and the required specialist contracts; arbitrary baseline references must remain rejected.

Task 5.5 can compose the already available local guard products with real candidate/queue/parallel/drift/cancellation fixtures. Its missing composed runner and evidence correspondence are local work, separate from the actual external identity/hosted-receipt requirements of production scenarios. The source pins and package capability matrix must be recorded for that composition rather than inferred from this example.

## Fixed source-bundle compatibility slice

`scripts/prepare-local-compatibility.py` prepares the specific reviewed GitGuard
source `df8d5ccd0445563577800ada0f6047a27444524c` with the existing reviewed
GuardEngine source bundle from `b0e4b445e1a20a9354d55ebb41bbedaf16ec8a74`:

```sh
python3 scripts/prepare-local-compatibility.py \
  /absolute/reviewed-guardengine-source.tar /absolute/new-review-directory
cd /absolute/new-review-directory/gitguard
cargo metadata --locked --offline --filter-platform x86_64-unknown-linux-gnu
cargo test --locked --offline --features privileged-execution \
  --test specialist_consumption --test evidence_trust \
  --test cli_outcomes --test rollout_rollback
cargo run --locked --offline --features privileged-execution --example local_rollout
```

Use `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1` and a controller-selected build target
outside the retained source directory. Preparation rejects a mismatched engine
archive and an existing output directory. It extracts only regular files and
directories within bounded archives, records the original source archives, and
changes only the GG manifest's engine path to `../guardengine-0.1.0`; the lock is
unchanged. The caller must protect the output directory. This reuses the existing
GE/TG source-bundle procedure; it is not a package registry or publication tool.

The fixed pins are deliberate. Later commits containing this preparation script
or documentation are **not** included in the prepared consumer's `df8d5cc` source.
The source archives' SHA256 values are:

- GitGuard: `0f4fbd6b03229a0fce95354fb5eab28dd5ca57df5f495c87cc3c2b9458fa614e`.
- GuardEngine: `6fa49f998a23803c4df4c47e9b7f55de32cb867e923364f711b6694980600d45`.

Author validation on Linux x86_64, Rust/Cargo 1.99.0: 8 specialist, 3 trust,
4 CLI and 1 rollout tests passed; the actual local rollout example exited 0.
Cargo metadata resolved the only local packages to these two bundled directories,
not live sibling source. The engine archive matched a fresh archive of the pinned
GE commit. Ledger `gitguard-package-verification.json` records manifest/lock/CLI
hashes and toolchain; `gitguard-package-tests.log`, `gitguard-package-paths.json`
and `gitguard-package-rollout.json` retain observed evidence. Preparation itself
sets `compatibility_verified=false`; only the separate verification record reports
the tests that actually ran.

This is a **source bundle**, not a verified current `.crate` release. The earlier
GE c80ec325 `.crate` (`da5b30c22c28c9db444a0ca9c214b9441c3b7aebd14c223606298547452189bc`)
lacks the current `validate_approval_record` API on source inspection; its old
package acceptance cannot qualify this current consumer. No old-package compile
or N/N-1 compatibility claim is made. Unfiltered offline metadata initially needed
uncached non-host `r-efi`; metadata was restricted to the tested Linux target,
without changing dependencies, downloading packages or claiming other platforms.

Task 5.4 remains unchecked: this proves a bounded independent-source consumer
combination, not public distribution, a complete release matrix, production
identity, hosted enforcement or the entire rollout task. Native observations in
the rollout remain the original retained captures.
