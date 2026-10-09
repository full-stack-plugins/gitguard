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

This remains **partial 5.5**: it does not claim two newly evaluated successful requirement pipelines, a synthetic merge-group joint scenario, cross-guard late-completion persistence, fresh native CodeGuard execution, or hosted receipt/real-identity enforcement. Existing dedicated tests cover some of those mechanisms separately; they are not counted as this composed scenario. The CLI process boundary avoids a reverse GitGuard-to-FlowGuard library dependency (FlowGuard already depends on GitGuard), and preserves producer dependency locks.
