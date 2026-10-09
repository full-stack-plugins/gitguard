# GitGuard

[English](README.md) · [简体中文](README.zh-CN.md)

**Git policy, change scope, concurrent-task analysis, exact merge candidates and safe Git operations for AI-native engineering.**

> **Documentation-only project.** At inspected main commit `e03b5fd06d8b3d81d4bbbd11ff0fb70bea00d485` (2026-10-09), this repository contains these two READMEs and two design documents. There is no CLI, MCP server, executable, manifest, test suite, schema or OpenSpec directory. All components, commands and delivery milestones below are proposals. The independent [gitflow-plugin](https://github.com/full-stack-plugins/gitflow-plugin) is an **unverified compatibility target**; its implementation and behavior were not inspected here.

## Problem and approach

Two agents can edit different files and still break the same API. A local hook can pass before the target branch advances. GitGuard is designed to bind approved change scope, actual Git objects, isolated task observations and verification to the **exact candidate that the protected merge mechanism will admit**.

~~~text
Approved requirement baseline + task scope + observed target OID
                               ↓
            Branch / Worktree / actual Diff checks
                               ↓
             Concurrent-task semantic impact warnings
                               ↓
        Immutable candidate / merge-queue group binding
                               ↓
               Domain facts → GuardEngine evidence
                               ↓
      FlowGuard gate + authenticated operation grant
                               ↓
          Trusted executor + protected atomic admission
~~~

## Boundaries and scenarios

GitGuard owns repository/commit identity, branch and worktree policy, actual change scope, candidate freshness and Git write safety. SpecGuard owns requirement approval meaning; ArchGuard owns architecture, CodeGuard code policy, TestGuard test evidence and FlowGuard lifecycle authorization. GuardEngine supplies neutral contract validation, rule evaluation and deterministic evidence computation; it does not approve or merge changes. The six guards are independent.

- **Parallel requirements:** isolate each task/worktree run; shared files, APIs or schemas produce conflict observations. A late run must never overwrite evidence for a newer candidate.
- **Merge queues:** check the actual queue candidate against its base and group identity. PR HEAD evidence cannot authorize a different merge candidate; target/group changes invalidate affected results.
- **Controlled writes:** previews are read-only; a separately authorized executor verifies a narrow, expiring grant and the expected target OID. Unknown write outcomes require remote reconciliation before retry.

Inputs are trusted task/scope and baseline references, local Git objects and explicit target snapshots. Outputs are proposed domain observations, immutable candidate references, protocol-compatible evidence and separate operation receipts. `ALLOW` is a scoped technical decision, not merge/release permission. Approval cannot make incomplete analysis or a tool failure acceptable.

Worktrees isolate directories, **not security privileges**. Protected branches, independent required checks and trusted executor identities must enforce admission. Contracts and CI policy must come from protected sources, not the candidate being checked.

## Protocol and delivery status

The shared current protocol is `guard.partme.ai/v1alpha1`: GuardContract YAML, GuardFacts JSON and GuardReport JSON with exact `forbid_relation`, strict fields, and `enforce`/`review`/`advise`. It has no Git candidate or authorization fields. GitGuard has no adapter yet. Verification is recomputation of unsigned evidence, not proof of identity or authorization.

The planned [integration contract](docs/integration-contract.md) keeps task/requirement/worktree/candidate bindings outside the existing wire format. The draft `guard.integration/v1alpha1` envelope is not parsed by the current engine. Future check commands target exits 0 `ALLOW`, 2 `BLOCK`, 3 `REQUIRE_APPROVAL`, 4 input/runtime/verification error; GitGuard currently implements none of them. Partial facts must yield `BLOCK` with `INDETERMINATE`, not a passing result.

## Documentation

- [Architecture, boundaries, concurrency and trust decisions](docs/architecture.md)
- [Technical design, contracts, recovery and measurable milestones](docs/technical-design.md)
- [Shared integration contract (draft)](docs/integration-contract.md)
- [GuardEngine protocol](https://github.com/full-stack-plugins/guardengine/blob/main/docs/protocol.md)

## Proposed CLI — not runnable

~~~sh
# Design examples only; no gitguard binary exists.
gitguard scope check --task TASK-104 --base main --head HEAD
gitguard conflict analyze --task TASK-104
gitguard merge preview --target main --head HEAD
~~~

Mutable ref arguments are convenience inputs only: the future implementation must resolve them once to immutable OIDs and record the resolution. Initial delivery will establish read-only observation and exact candidate fixtures, then evaluate legacy compatibility with pinned provider versions and differential tests. No force-push, destructive reset, branch deletion, hidden fetch or policy-script execution belongs in a read-only check.
