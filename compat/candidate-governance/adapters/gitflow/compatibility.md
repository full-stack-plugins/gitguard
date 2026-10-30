# Gitflow read-only compatibility survey

This opening section records the historical research checkpoint, before the opt-in importer described below. Native Gitflow entrypoints remain unchanged. GitGuard does not auto-run Gitflow, invoke `--apply`, or translate its exit status into authority.

Official source: [full-stack-plugins/gitflow-plugin](https://github.com/full-stack-plugins/gitflow-plugin/tree/310c43c5e3fe7effa19245dcc4970249e569004c), commit `310c43c5e3fe7effa19245dcc4970249e569004c`, plugin manifest version `0.2.0`. Read-only checkout fetched into `/tmp/gitguard-gitflow-survey-310c43c`; script verifies HEAD and clean worktree before execution. The manual probe creates a disposable real repository, runs only native check, then runs the actual GitGuard CLI on the same candidate and verifies all fixture file bytes remain unchanged. Credentials and global/system Git config are excluded from fixture execution.

Reproduce after fetching the pinned official checkout and building GitGuard:

```sh
python3 adapters/gitflow/survey.py --provider-root /tmp/gitguard-gitflow-survey-310c43c --gitguard-bin target/debug/gitguard
```

| Probe | Observed native exit | Observed decision |
|---|---:|---|
| feature/task to develop | 0 | allow |
| same candidate to main | 1 | deny |
| nonexistent immutable head object | 3 | unverified |
| unknown CLI flag | 2 | error |
| GitGuard same candidate, path a allowed | 0 | ALLOW |

Raw reproducible observations are in `observed.json`. Source bytes remained unchanged. This establishes that numeric exit passthrough is wrong: native deny=1/unverified=3/usage=2 differ from GitGuard BLOCK=2/REQUIRE_APPROVAL=3/error=4.

Source inspection: [cli.py](https://github.com/full-stack-plugins/gitflow-plugin/blob/310c43c5e3fe7effa19245dcc4970249e569004c/scripts/gitflow/cli.py) maps successful allow/preview/applied to 0, deny to 1, unverified to 3 and internal error to 4; parser usage has its own exit 2. Internal error exit 4 was source-inspected only, not exercised by this probe. `--json` controls compact formatting; reports already use the native JSON result schema. [git.py](https://github.com/full-stack-plugins/gitflow-plugin/blob/310c43c5e3fe7effa19245dcc4970249e569004c/scripts/gitflow/git.py) supplies schema_version/action/decision/reasons/next_actions and bounded local subprocess behavior. [ci.py](https://github.com/full-stack-plugins/gitflow-plugin/blob/310c43c5e3fe7effa19245dcc4970249e569004c/scripts/gitflow/ci.py) resolves full commit IDs, reads workflow policy at the fixed base, rejects shallow/unrelated/missing-object cases as unverified, and evaluates branch/message/author/tag policy without fetching. Server protection is not proven by local checks.

The evaluators have different semantics: Gitflow checks workflow/branch policy, while the current GitGuard profile checks frozen byte-path scope. One agreeing ALLOW is not equivalence. At that historical checkpoint task 3.6 remained partial: timeout/malformed provider output/internal-error differential fixtures, an explicit opt-in adapter, complete status translation and migration acceptance were deferred. The research script is not a production untrusted-provider sandbox and requires a reviewed clean pinned checkout.

## Opt-in compatibility importer (current slice)

`gitguard::gitflow::PreparedCheck` is an explicitly called library importer; no default registration, process launcher, CLI routing, provider replacement or executor authorization is enabled. The historical survey above remains intact as earlier evidence. The new `capture.py` and `fixtures/` retain actual unchanged-provider outputs for allow0/deny1/unverified3/usage2/internal4. Internal4 uses a separately labelled fault-only `--message-file` containing invalid UTF-8; normal prepared profile is fixed `check --base <base> --head <head> --source <source> --target <target> --json`, with commits message mode. No apply/fetch/push is executed.

Prepare validates an actual local candidate, freezes its entire binding, verifies controller-supplied raw workflow bytes against the immutable base's regular `.gitflow/workflow.json` blob, and freezes both its raw digest and the controller's independently declared normalized policy hash. The capture computes the normalized hash from the pinned provider's policy loader before collecting results, rather than copying it from the result. The Rust importer does not replicate the provider's normalization rules or authenticate the controller's hash declaration. Fixed immutable commit-range IDs and ordering are also frozen (maximum500). Raw policy/borrowed expectation/report budgets are1MiB; native source/target names are bounded literal components. Candidate changes cannot reuse an earlier prepared import.

The importer accepts only schema1.0.0/actioncheck with matching native exit/status. Complete allow/deny records must match base/head/source/target/policy reference/hash and complete commit IDs/count; allow additionally requires all fixed native check IDs, compatible check statuses and no reported reasons. Duplicate JSON members, unknown fields/schema, missing scope, contradictory success and oversized input fail closed. Native unverified3 is an explicitly incomplete local observation, **not REQUIRE_APPROVAL/REVIEW3**. Usage2/internal4 and consumer timeout/parser failure return errors with no successful decision. Public observations expose only outcome and raw/expectation hashes, not executable native advice or diagnostic text.

`Compatible` is not a GuardEngine result, mandatory-gate satisfaction, server protection, producer authentication or execution permission. Parsing bytes cannot establish that a provider ran, which source produced them, or that a caller's native exit/timeout is truthful. This slice is a local controller compatibility boundary; a separately authenticated runner/envelope would be needed for production consumption. Diagnostic unverified output may omit bindings, remains incomplete, and never supplies success evidence.

### Actual same-candidate differential

`fixtures/source.bundle` retains the deterministic actual Git history. Native captures use the same base/head as both real GitGuard CLI captures. The source tree, index and refs were checked byte-for-byte unchanged. The differing dimensions are independent protected policies, not attempts to prove semantic equivalence:

| Gitflow target | GG allowed path | Native workflow | GG path scope | Migration disposition |
|---|---|---|---|---|
| develop | a | allow0 | ALLOW0 | Agreement only for these independent predicates |
| main | a | deny1 | ALLOW0 | Workflow rejects merge target; path guard cannot replace it |
| develop | b | allow0 | BLOCK2 | Path guard rejects changed a; workflow cannot replace it |
| main | b | deny1 | BLOCK2 | Both reject for different reasons |

Retain both policy owners and required checks; never silently switch ownership based on numeric statuses or an agreeing sample. Unknown/error supplies neither owner's missing success.

`transport-timeout.json` is explicitly a **consumer transport injection**: a real stopped launcher reaches a deadline, is killed and reaped before native execution. It is not a naturally occurring provider timeout or native error report. Malformed JSON and contradictory report cases in the Rust tests are labelled consumer mutations of real captures, never claimed as native output. Collector operates only on its generated bounded fixture and pinned trusted checkout; it is not an untrusted-provider sandbox.

The identical nonexistent all-zero head also runs through actual GG CLI: native Gitflow emits unverified3, while GG emits prebinding error4. Neither is review/approval or success; keep native incompleteness distinct from GG preparation error. Native usage2/error4 are captured with their own fault-only arguments; GG's valid-candidate path ALLOW does not establish Gitflow message-file decoding success. They have no policy-equivalence translation.
