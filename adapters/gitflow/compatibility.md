# Gitflow read-only compatibility survey

This is research, not an installed adapter or completed differential acceptance suite. Native Gitflow entrypoints remain unchanged. GitGuard does not auto-run Gitflow, invoke `--apply`, or translate its exit status into authority.

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

The evaluators have different semantics: Gitflow checks workflow/branch policy, while the current GitGuard profile checks frozen byte-path scope. One agreeing ALLOW is not equivalence. Task 3.6 remains partial: timeout/malformed provider output/internal-error differential fixtures, an explicit opt-in adapter, complete status translation and migration acceptance are deferred. The research script is not a production untrusted-provider sandbox and requires a reviewed clean pinned checkout.
