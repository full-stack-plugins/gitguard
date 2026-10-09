# GitGuard

[English](README.md) · [简体中文](README.zh-CN.md)

**Read-only Git candidate observation, frozen change scope and candidate-bound evidence.**

This implementation branch contains a Rust library, a `gitguard check` CLI, schemas and real Git tests. It is no longer documentation-only. **13/28 OpenSpec tasks are independently accepted for their documented local profiles**; newer work remains pending review. See [implementation progress](docs/implementation-progress.md) and [tasks](openspec/changes/add-candidate-bound-git-governance/tasks.md) for precise acceptance boundaries.

## Implemented local capabilities

- Real SHA-1/SHA-256 objects, linked worktree common directories, commit/tree/index/worktree distinction, full immutable OIDs and byte-safe path scope, including both rename endpoints.
- Private temporary two-parent merge previews, exact candidate/base/group/member bindings and isolated task/worktree leases. Dirty sources and missing submodule/graph coverage remain explicit uncertainty.
- Actual GuardEngine projection, bounded evaluation and artifact verification. `gitguard check` reads one strict JSON request on stdin and writes the current bundle to stdout: 0 ALLOW, 2 BLOCK, 3 REQUIRE_APPROVAL, 4 failure. There is no `--report` or mutable-ref convenience command.
- Explicit trust-provider ports, full-key-bound consumption and process-local append-only history/CAS. A separate restricted Linux local-file backend provides tested restart and multiprocess behavior; its review status is recorded separately. Neither store turns historical eligibility into current authority.

The semantic graph and authority implementations under tests are labelled fixtures. **No production authority provider, approved non-null baseline, protected hosted admission, MCP server, remote push/merge or operation-intent executor exists.** Every writer operation remains denied, including feature-enabled builds. ALLOW is a technical result, not merge permission. Approval cannot repair partial/error evidence or rewrite REQUIRE_APPROVAL.

Observation currently requires controlled local checkouts; hostile concurrent filesystem replacement and full OS resource containment are not proven. Repository labels are assigned by the caller, never authenticated from origin URLs. [Observation limits](docs/git-observation-adr.md), [history limits](docs/local-evidence-store-adr.md) and [Linux persistence limits](docs/durable-history-adr.md) are part of the supported contract.

## Build and run

Development uses a sibling `../guardengine` source checkout and Cargo.lock; this is not an independently published package. Rust 1.90 is the declared floor. The verified toolchain/Git matrix is recorded in the observation ADR.

```sh
cargo test --locked
cargo run --locked -- check < request.json
```

Construct `request.json` according to the [actual CLI contract](docs/git-engine-cli-contract.md), using real immutable candidate/base OIDs and a scope policy digest derived from `FrozenPolicy`. [CLI tests](tests/cli_outcomes.rs) construct real requests. There are no `scope check`, `conflict analyze` or `merge apply` commands.

For a complete real-object producer fixture and exact artifact bytes:

```sh
cargo run --locked --example export_evidence -- /absolute/path/to/new-golden-directory
```

This creates a disposable repository, runs the actual producer and exports contract/facts/report/envelope/domain, the complete bundle and the source Git bundle. It does not authenticate the fixture as production evidence.

## Protocol and remaining work

Native `guard.partme.ai/v1alpha1` remains unchanged: strict GuardContract/GuardFacts/GuardReport and exact `forbid_relation`. Actual integration uses the separate `guard.integration/v1alpha1` envelope and eligibility ports; candidate bindings remain in GitGuard's `gitguard.candidate/v1alpha2` domain. Hash/recomputation verification does not prove issuer identity.

The official gitflow-plugin was inspected and exercised read-only at a pinned commit: [compatibility survey](adapters/gitflow/compatibility.md). Its exit semantics differ from GitGuard. No opt-in adapter or replacement claim is enabled; native interfaces are unchanged.

Remaining work includes protected baseline provenance, production provider integration, further process/filesystem hardening, full Gitflow differential adaptation, versioned MCP/API, hosted protections and separately reviewed write authorization/recovery. [Architecture](docs/architecture.md) and [technical design](docs/technical-design.md) describe this broader target; their future capabilities are not current product claims. [Integration contract](docs/integration-contract.md) and [dependency roadmap](openspec/guard-roadmap.md) explain shared boundaries.
