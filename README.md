# GitGuard

[English](README.md) · [简体中文](README.zh-CN.md)

**Git policy, change scope, parallel-task conflict analysis, final merge candidate and branch governance for AI-native engineering.**

> **Status: detailed architectural and technical blueprints are written. This new GitGuard repository does not yet ship an executable CLI or an enforced merge service.** Existing Git capabilities are maintained separately in [gitflow-plugin](https://github.com/full-stack-plugins/gitflow-plugin).

## Problem and approach

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
