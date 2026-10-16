# GitGuard

[English](README.md) · [简体中文](README.zh-CN.md)

**Git policy, change scope, parallel-task conflict analysis, final merge candidate and branch governance for AI-native engineering.**

> **Status: buildable agent-integration CLI and read-only MCP are available in source.** An explicitly trusted adapter reads active GitFlow rules; Git writes and server-enforced merge gates are not implemented.

## Problem and approach

Implemented professional checks: `policy check --action branch|merge|commit` and MCP `gitguard_policy_check` reuse the active GitFlow policy without Git writes. See [plugin usage, trust boundary and verification](plugin/README.md). `allow` is a policy decision; technical quality and approvals remain unevaluated.

Two agents may edit different files but break the same public API. A local Git hook can pass while the target branch moves, invalidating the checked merge result. GitGuard is designed to bind each change to an approved task, permitted change scope, actual Git refs, and the **exact final merge candidate**.

~~~text
Approved task/scope + Target ref
                ↓
    Branch / Worktree / Scope checks
                ↓
   Diff + semantic-impact observations
                ↓
       Final merge candidate
                ↓
          GuardEngine evidence
                ↓
   FlowGuard approval + trusted executor
                ↓
  Protected CI / atomic target-ref verification
~~~

## Domain ownership

GitGuard owns branch rules, change scope, repository and commit identities, semantic conflict warnings, target OID freshness and Git write-operation safety. It does **not** replace ArchGuard design analysis, CodeGuard code checks, TestGuard behavior tests, SpecGuard requirement approval, or FlowGuard lifecycle policy.

Merge enforcement requires the actual Git hosting service's protected branches, independent required checks and properly scoped executor identities. Worktrees isolate directories, **not security privileges**. Ambiguous or failed Git writes require remote reconciliation before retry.

## Documentation

- [System architecture, trust boundaries and ADRs](docs/architecture.md)
- [Technical blueprint, data/CLI/CI interfaces and delivery Waves](docs/technical-design.md)
- [GuardEngine Protocol](https://github.com/full-stack-plugins/guardengine/blob/main/docs/protocol.md)

## Planned CLI (not executable yet)

~~~sh
gitguard scope check --task TASK-104 --base main --head HEAD
gitguard conflict analyze --task TASK-104
gitguard merge preview --target main --head HEAD
~~~

GitGuard will initially adapt and validate existing GitFlow policy behavior, then migrate rule ownership only after differential tests. Never run force-push, destructive resets, branch deletion, or hidden fetches during read-only checks.

See [GitHub repositories](https://github.com/orgs/full-stack-plugins/repositories).

## Agent installation and managed instructions

The shared installer provides `install`, `uninstall`, `context`, `instructions render|sync`, `doctor --integration`, and `serve --mcp` for Codex, Claude Code, and Gemini CLI. Each tool renders its own managed block.

```sh
cargo build --locked
gitguard install --agents codex,claude,gemini --project /absolute/project --bin-dir /absolute/bin --dry-run
gitguard context --project /absolute/project
```

After building, invoke the executable under `target/debug/` (with `.exe` on Windows); short commands above assume PATH is configured. `--bin-dir` copies the current executable; PATH is not changed automatically. Remove `--dry-run` to apply. A sibling `../guardengine/crates/guard-install` source checkout is required.

Dynamic rules require explicit `--gitflow-root <trusted-plugin> --python <existing-executable>` for install, context and sync. MCP configuration pins these arguments. Candidate drift is unverified; an absent adapter is unconfigured.

[Shared design, host mapping, update/uninstall and verification](https://github.com/full-stack-plugins/guardengine/blob/main/docs/agent-installation.md). Configuration success does not prove host loading or quality approval.


Read-only policy/context queries check effective Git `filter.*.clean` and `filter.*.process` configuration before requesting status, on both platforms. Any nonempty effective command returns `unverified` (`executable_filter_unverified`) without starting the filter, including inherited Git LFS commands. This intentionally conservative restriction requires an isolated environment or separately authorized workflow; the tool does not remove filters or substitute a fabricated clean status. Windows preserves native `GIT_AUTHOR_*`/`GIT_COMMITTER_*` identity variables while removing repository/config routing overrides. Optional index refresh writes are disabled on both platforms. This is local configuration inspection, not a sandbox against concurrent repository/config mutation.
