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

Original task 5.4 is independently accepted for the local rollout, required-check preservation and aligned fixed-source package profile. Remaining deployment work is separate: establish protected hosted rollout ownership, deployment-specific native differential adjudications and actual hosted enforcement. Public distribution and hosted required-check behavior are not established by the local example.

Task 3.3's specialist verification port is independently accepted at `8b87946f`; task 5.5's four-group real producer consumption is independently accepted at `5207e9e`. Their retained captures and frozen expectations are now reused below. This does not add a production identity provider or make domain baselines interchangeable with the Git candidate baseline.

## Accepted required specialist checks inside the local write caller

`local_rollout::run` also invokes `required_specialist_rollout`, which reuses the existing joint consumer's M2 group and private local writer. This second report has its own exact candidate, source bundle and five required domains. It is separate from the older Gitflow candidate/profile above: captured M2 evidence is never relabelled as Gitflow evidence. The smaller callable fixture restores only M2; it does not replay the four-group negative matrix or rerun any producer.

`PreparedSpecialistConsumption` is frozen once before dispatch. Every actual local write dispatch calls its fresh `consume` against the **apply context's** repository/candidate, complete required envelope set, immutable original artifacts and explicit fixture authority before entering the existing grant/apply port. There is no cached eligible Boolean or new grant format. This is a current consumption-time check, not an atomic promise about future authority state across external providers; apply retains its separate grant refresh and expected-target CAS.

Five actual dispatch negatives use otherwise valid operation authority: missing TestGuard, revoked producer, expired producer, revoked SpecGuard baseline, and disabled CodeGuard adapter. Each returns RequiredSpecialists with zero grant calls and identical full target/intent bytes. Disabling an adapter does not reduce the frozen five-domain requirement. Restoring valid evidence still does not make a fake FG-ALLOW reference an operation grant. Valid independent fixture authorization then performs the actual M2 local CAS. Cancellation and revoked operation grants retain their existing behavior; executor rollback still permits read-only specialist checks and preserves reopened Applied/RecoveryRequired journal history. SpecGuard's original REQUIRE_APPROVAL remains unchanged.

Run only this new slice with `cargo test --locked --offline --features privileged-execution --test rollout_rollback required_specialists_guard_actual_m2_rollout_dispatch -- --exact --nocapture`. The JSON records exact candidate, actual target, operation/request digest, all required scopes and five denied-dispatch observations; source/corpus hashes are checked before/after. The retained transport pin is unchanged. Local fixture authority is not production identity, and no hosted write, public release or new phase controller is introduced. The df8d5cc historical package does not contain this increment; the separately retained 85a9743 package below does. Original 5.4 is accepted for this explicit local profile.

## Fixed source-bundle compatibility slice

`scripts/prepare-local-compatibility.py` now fixes GitGuard
`85a9743023de788d91a370601ad2b7d5a649517a`, including the required-specialist
M2 dispatch entry, with the same reviewed GuardEngine source bundle
`b0e4b445e1a20a9354d55ebb41bbedaf16ec8a74`:

```sh
python3 scripts/prepare-local-compatibility.py \
  /absolute/reviewed-guardengine-source.tar /absolute/new-review-directory
cd /absolute/new-review-directory/gitguard
cargo metadata --locked --offline --format-version 1 --filter-platform x86_64-unknown-linux-gnu
cargo test --locked --offline --features privileged-execution \
  --test rollout_rollback required_specialists_guard_actual_m2_rollout_dispatch \
  -- --exact --nocapture
```

Use `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1` and a controller-selected build target
outside the retained source directory. Preparation rejects a mismatched engine
archive and an existing output directory. It extracts only regular files and
directories within bounded archives, records both original archives, and changes
only the GG manifest's engine path to `../guardengine-0.1.0`; the lock is unchanged.
The caller must protect the output directory. The package contains precisely the
fixed source, not later preparer/documentation commits. This is local source
packaging, not a package registry or publication tool.

The new artifact is ledger `gitguard-local-source-85a9743.tar.gz`, SHA256
`076d37b1d48fe214139f433348bf3ce5c267ce201fe7762a50e50ddb5784d643`.
Original source archive hashes:

- GitGuard: `adb08b244f6710d2752bd7f582cce2559769ff3da479ae1e979df62508ebbcfe`.
- GuardEngine: `6fa49f998a23803c4df4c47e9b7f55de32cb867e923364f711b6694980600d45`.

Author validation from inside this independent package: the new M2 required-check
rollout test passed (one test, five refusal cases plus real authorized local write
and rollback). Cargo metadata resolves its only local packages to the two bundled
directories. All 577 GG and 251 GE regular files match their retained source
archives, with only the declared GG manifest path transformation. Evidence:
ledger `gitguard-package-85a9743-{prepare,metadata,verification,capture}.json` and
`gitguard-package-85a9743-tests.log`. The preparer itself continues to write
`compatibility_verified=false`; only the separate execution record reports the
checks actually performed. No old full matrix or native producer was rerun.

The previous fixed `df8d5ccd0445563577800ada0f6047a27444524c` package and all of
its evidence remain retained unchanged. Its GG source archive SHA256 is
`0f4fbd6b03229a0fce95354fb5eab28dd5ca57df5f495c87cc3c2b9458fa614e`.
That earlier package passed 8 specialist, 3 trust, 4 CLI and 1 rollout tests and
an actual local rollout run. These are historical checks of that exact version,
not a claim that the new M2 entry existed in the older artifact.

Both are **source bundles**, not verified current `.crate` releases. The earlier
GE c80ec325 `.crate` (`da5b30c22c28c9db444a0ca9c214b9441c3b7aebd14c223606298547452189bc`)
lacks the current `validate_approval_record` API on source inspection; its old
package acceptance cannot qualify this consumer. No N/N-1 or public distribution
claim is made. Metadata is limited to the tested Linux host without downloading
non-host dependencies or changing the dependency graph.

Task 5.4 is accepted after independent fixed-source and package review. See ledger `gitguard-required-rollout-independent-review.md` and `gitguard-package-85a9743-independent-review.md`. This aligned
local source/package combination does not establish production identity, hosted
required-check enforcement or atomic future authority across external providers.
Native observations remain the original retained captures.
