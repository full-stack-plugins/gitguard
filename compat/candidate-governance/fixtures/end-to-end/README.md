# Retained five-producer gate replay

This opt-in task 5.5 slice composes current GitGuard/FlowGuard command consumers with **unchanged actual producer artifacts** from FlowGuard's reviewed approval-bridge corpus. `actual/PROVENANCE.json` retains the original SG/AG/CG/TG/GG producer source pins, original commands, binary hashes, protected input hashes and captured artifact hashes. Its fixed SHA256 is `0be2683c5afa2be89c2b528a3316fcf733b3f91bc0135fd7bd4394808260c743`; all listed bytes are checked before execution. This does not rerun those producers or re-establish CodeGuard native parity.

The captured real Git candidate is `10765def2dc06ff914c5e88df9ac5834d607e3ae`, base `88ac090f58453bd5f095bfb844f539ec60ad26a3`. The script restores its bundled repository privately, runs current GitGuard `check`, and requires the entire fresh CandidateSnapshot to equal the retained GitGuard snapshot. FlowGuard then validates the five original contract/facts/report/envelope sets using their distinct policy owners, scope IDs and source digests. SG's approved-baseline context remains in FlowGuard's existing scoped-source contract; GitGuard's non-null-baseline rejection is unchanged.

Run with explicit controller-selected binaries and their SHA256 values:

```sh
python3 fixtures/end-to-end/run.py \
  --gitguard /absolute/gitguard --gitguard-sha256 HASH \
  --flowguard /absolute/flowguard --flowguard-sha256 HASH \
  --output /absolute/new-capture-directory
```

The output directory must not already exist. It retains raw original inputs, the current GitGuard request/output, each FlowGuard request, fixture authority record, raw stdout/stderr, and a result with byte digests and actual exits. The temporary repository is removed; the original bundled source remains available for replay. A new run rewrites only its freshly constructed request's local repository path, never original producer artifacts.

Tool hashes pin executable bytes, not identity, signatures or source-to-binary provenance. The opened executable descriptor is retained through execution to avoid path replacement; the controller must protect the executable against in-place writes. Commands have fixed argv, cleared environment credentials and deadlines; this is an explicitly selected trusted-tool fixture runner, not a sandbox for arbitrary programs. No API calls, credentials, hosted checks, Git writes or production authority are involved.

The fixture authority is newly generated, separately recorded and explicitly labelled `local_fixture`. It is not a recovered original signed identity. Positive gate completion still reports `execution_authorized=false`. The expiry/revocation cases exercise SG review approval records; they do not freshly validate the retained SG baseline authority or establish a baseline authentication/freshness contract for GitGuard. Cases cover:

- Current same-candidate five-producer gate, with SG's technical REQUIRE_APPROVAL left unchanged.
- Missing, expired or revoked referenced approval records: actual current FlowGuard bound error/null decision/exit 4.
- Missing specialist evidence: actual BLOCK/exit 2.
- Cancellation: null decision/exit 4.
- Concurrent mismatched task and differing candidate requests: pre-binding refusal/exit 4, never cross-satisfying another context.
- Exact source and retained artifact bytes unchanged after all runs.

The opt-in Rust test is ignored by default because an independently selected FlowGuard executable is required:

```sh
GITGUARD_BINARY=/absolute/gitguard FLOWGUARD_BINARY=/absolute/flowguard \
  cargo test --locked --offline --test end_to_end_guard_gates -- --ignored
```

This earlier eight-case replay alone is a **partial 5.5 slice** (the later four-group acceptance is recorded below): it does not claim two newly evaluated successful requirement pipelines, a synthetic merge-group joint scenario, cross-guard late-completion persistence, fresh native CodeGuard execution, or hosted receipt/real-identity enforcement. Existing dedicated tests cover some of those mechanisms separately; they are not counted as this composed scenario. The CLI process boundary avoids a reverse GitGuard-to-FlowGuard library dependency (FlowGuard already depends on GitGuard), and preserves producer dependency locks.

## GG consumption of the four independently generated groups

```sh
cargo test --locked --offline --features privileged-execution --test joint_specialist_consumption
cargo run --locked --offline --features privileged-execution --example joint_consumption
```

The GG-only example accepts no user repository, URL or credential. It restores a
fixed bundle into a private temporary repository under the working directory,
creates four detached physical worktrees, and uses the real
`PreparedSpecialistConsumption` port. Run on the already supported local filesystem
profile; intent storage does not accept tmpfs. Default builds reject the mutation
fixture before creating it. There is no FlowGuard library dependency.

`joint/` contains unchanged material from the independent FG joint capture produced
with FG `1fd70a11ac0e874ca1bf21801987320c237647de`; its
`independent-source-and-artifact-verification.json` retains the exact other source
pins. `producer-manifest.json` pins all 304 original producer artifacts across
A/B/M1/M2. The new `TRANSPORT.json` pins copied capture files plus an added Git
transport bundle; its own SHA256 is fixed in the example. The bundle was made from
a separate temporary bare copy of the captured repository with explicit references
to all four captured OIDs. This neither changes original envelopes nor reruns any
producer/native command. Original absolute paths remain historical metadata;
the example never reads those locations.

A/B are different real tasks and requirement sets with the same project baseline
graph/source revision and different approved scope digests. M1/M2 are different
real synthetic commits, group IDs, member orders and bases. The original logical
worktreeId is `fixture-worktree` in all four captures; that value is preserved,
while the replay creates four separate physical worktrees. Policy expectations
are read from separately pinned per-domain policy artifacts, never inferred from
received envelopes or flattened into one domain's scopes.

The example verifies:

- Concurrent A/B consumption and positive consumption of A/B/M1/M2, including each
  non-null domain baseline. Technical SG REQUIRE_APPROVAL remains unchanged.
- Per group: expiry, baseline revocation, review revocation, missing required
  domain, and separately changed **same SG domain** baseline/source/contract/analyzer
  expectations all reject. The JSON includes actual per-domain eligibility codes
  and technical decisions for these 32 negative evaluations.
- A→B, A→M1 and M1→M2 evidence never cross-satisfy the new frozen expectation.
- Existing GG `LocalHistory` accepts M2's real original GG bundle, then archives
  M1's late completion but refuses its publication. This is actual GG self-bundle
  history, not a new persistence API for aggregate specialist consumption.
- Under the same M2 candidate, retained FG ALLOW and new GG eligibility cannot
  substitute for an operation grant: the real `apply` call rejects the unavailable
  grant reference without changing target bytes. An already-cancelled call with a
  separate valid fixture grant also changes no target bytes and creates no intent.
- Only a subsequent explicitly authenticated fixture operation grant advances the
  isolated bare target. Another claimed operation loses authority and stays
  RecoveryRequired. Disabling the executor rejects writes; reopened Applied and
  unknown records and read-only eligibility survive unchanged.
- Original captured files, the restored source repository and all four physical
  worktree bytes are unchanged. The JSON records source/worktree hashes, actual
  M2 target OID, operation ID, request digest, audit records and observed outcomes.

Original FG fixture producer/review authority records are retained. GG's new
baseline-approval port needs **additional new fixture-only records** binding each
non-null domain baseline; these are not reconstructed production approvals or
claims to authenticate the historical captures. Artifact SHA256 and stored ALLOW
never become authority. Current CodeGuard native execution/parity is not rerun.

The retained first-capture `invalidation.stdout.json` is historical context only;
this example does not execute or rely on its original drift helper. GG's drift
cases directly change independently frozen expectations in the same domain. FG's
separately corrected drift helper at `7b3c3f4` has its own independent evidence and
is not silently substituted into these retained bytes.

Task 5.5 is independently accepted for this local joint profile at
`5207e9e2dff367856f052f530258adce2351ae14`, without claiming hosted queue identity, real remote
receipts, production rollout, fresh native execution, aggregate-specialist durable
history, or a full new crash matrix for all four candidates. The six-process crash
matrix and adapter rollout exercise remain their separate accepted slices; this
example exercises same-M2 cancellation, local effect, executor disable and durable
intent reopening. The review combines the independently executed FG producer/native
capture with this actual GG consumer run; retained CodeGuard native/schema/sidecar
comparison is accepted evidence, without claiming the GG example reruns it.

The reviewer ran the actual example from a fixed GG source archive against GE
`b0e4b445e1a20a9354d55ebb41bbedaf16ec8a74`: exit 0, four positive groups,
32 within-group negatives, three cross-context refusals and same-M2 mutation
boundaries passed. All 340 transport files and all 304 original producer artifacts
were independently checked; all 37 unique reachable Git objects matched the original
capture. Evidence: ledger `gitguard-joint-specialists-independent-review.md` and
`gitguard-joint-specialists-independent-example.{json,log}`. GitGuard is 24/28;
this acceptance does not deploy production identity/grants or hosted enforcement.
