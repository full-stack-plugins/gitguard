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
