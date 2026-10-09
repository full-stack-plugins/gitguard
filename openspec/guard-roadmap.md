# Guard cross-repository OpenSpec roadmap

Status: **pending implementation planning**, 2026-10-09. These seven incremental changes implement the reviewed architecture/technical designs; writing or structurally validating a change does not complete its runtime tasks. All new tasks start unchecked. This roadmap is identical in all seven repositories and supplements [shared integration contract](../docs/integration-contract.md).

## Change ownership

| Repository / change | Owned work | Existing work preserved |
|---|---|---|
| [GuardEngine / add-versioned-guard-integration-contracts](https://github.com/full-stack-plugins/guardengine/tree/docs/guard-design-20261009/openspec/changes/add-versioned-guard-integration-contracts) | Shared schema/capabilities, generic adapters, evidence eligibility ports and pinned distribution | bootstrap-guard-protocol keeps implemented wire/evaluator history; detailed new integration tasks refine its future umbrella TODOs |
| [SpecGuard / add-specification-baseline-analysis](https://github.com/full-stack-plugins/specguard/tree/docs/guard-design-20261009/openspec/changes/add-specification-baseline-analysis) | Source graph, requirement identity, authenticated baseline diff and traceable obligations | No runtime or earlier OpenSpec at inspected baseline |
| [ArchGuard / extend-architecture-analysis-and-evidence](https://github.com/full-stack-plugins/archguard/tree/docs/guard-design-20261009/openspec/changes/extend-architecture-analysis-and-evidence) | Current analyzer hardening, deeper architecture analysis and scoped interoperable evidence | add-cargo-workspace-guard keeps current implementation and historical task states; new granular work references its future umbrella TODOs |
| [CodeGuard / add-guardengine-compatibility-adapter](https://github.com/full-stack-plugins/codeguard/tree/docs/guard-design-20261009/openspec/changes/add-guardengine-compatibility-adapter) | Opt-in evidence projection and interoperability, preserving native commands/reports | introduce-rust-codeguard-cli remains sole owner of native languages, CLI, repair and distribution backlog; do not duplicate its implementation tasks |
| [TestGuard / add-test-obligation-evidence-pipeline](https://github.com/full-stack-plugins/testguard/tree/docs/guard-design-20261009/openspec/changes/add-test-obligation-evidence-pipeline) | Frozen test obligations, execution/coverage, isolation and evidence | No runtime or earlier OpenSpec at inspected baseline |
| [GitGuard / add-candidate-bound-git-governance](https://github.com/full-stack-plugins/gitguard/tree/docs/guard-design-20261009/openspec/changes/add-candidate-bound-git-governance) | Read-only candidate binding first; evidence-gated Git operations only after controller authority | No runtime or earlier OpenSpec at inspected baseline |
| [FlowGuard / add-evidence-bound-workflow-gates](https://github.com/full-stack-plugins/flowguard/tree/docs/guard-design-20261009/openspec/changes/add-evidence-bound-workflow-gates) | Stage graph, obligation composition, approval eligibility and invalidation | External legacy plugin behavior is unverified until fixed-revision compatibility investigation |

## Dependency order and independently testable gates

| Order / gate | Prerequisites | Required acceptance artifact |
|---|---|---|
| 0: source/compatibility survey | None | Each Guard's source-backed scope and unresolved decisions; native fixtures and read-only discovery may start in parallel |
| 1: GE-CONTRACT | GuardEngine tasks 1.x, reviewed consumer mappings | Frozen envelope schema/profile/version and positive/negative vectors; no claim of current wire extension |
| 2: GE-ADAPTER | GE-CONTRACT | Opt-in generic mapping with native engine/ArchGuard/CodeGuard parity and error-versus-partial distinction |
| 2: SG-BASELINE | SpecGuard source identity/graph/baseline design; integrated export additionally GE-CONTRACT/GE-ADAPTER | Stable requirement/acceptance IDs, immutable baseline validation ports/labelled fixture vectors and versioned test-obligation export; authoritative production baseline consumption additionally waits GE-TRUST |
| 2: GG-CANDIDATE | GitGuard read-only Git/object identity fixtures; envelope export uses GE-CONTRACT/GE-ADAPTER | Verified exact candidate/base/merge-group binding and safe read-only preflight; no privileged writer needed |
| 3: GE-TRUST | GE-ADAPTER | Generic authenticated-record ports, expiry/revocation/freshness and concurrent immutable publication fixtures; authority remains external |
| 3: AG-EVIDENCE | GE-CONTRACT/GE-ADAPTER; SG-BASELINE only for requirement/ADR trace features | Scoped analyzer evidence with actual language/capability acceptance, no empty-facts fallback |
| 3: CG-ADAPTER | GE-CONTRACT/GE-ADAPTER and supported native command evidence from existing CodeGuard backlog | Versioned opt-in projection and parity; incomplete native scope stays incomplete, no need to finish unrelated native backlog first |
| 3: TG-EVIDENCE | GE-CONTRACT/GE-ADAPTER; SG-BASELINE for requirement-based plans | Executed obligation×environment fixtures, honest coverage/failed/flaky evidence; authoritative use additionally GE-TRUST |
| 4: GE-RELEASE | GuardEngine tasks 4.1–4.2, supported compatibility profiles | Pinned independent artifact and tested compatibility matrix; later joint acceptance is not a prerequisite for this limited release gate |
| 4: FG-GATE | SG-BASELINE, required AG/CG/TG evidence, GG-CANDIDATE, GE-TRUST | A new scoped FlowGuard report with immutable upstream reports and external approval records, no execution authority |
| 5: privileged Git execution (optional) | Read-only GG-CANDIDATE, required evidence/FG-GATE where configured, separately reviewed controller authority | Expiring action-scoped grant, compare-and-set refs, recovery and no duplicate writes |
| 5: END-TO-END | Relevant gates above; independent production consumption also GE-RELEASE | Two requirements, exact synthetic queue candidate, drift/expiry/revocation/late-result and migration rollback fixtures |

Numbers express a partial order, not a requirement to finish every project at each level. Native parser/fixture work can proceed before SDK integration. CodeGuard native-only evidence may be carried only under an explicit weaker profile; it cannot fulfill an engine-backed complete obligation. If a native capability is not ready, the dependent gate remains unavailable rather than manufacturing ALLOW.

SG-BASELINE initially proves the local domain contract and explicitly labelled fixture/port behavior, not production approval authentication. Every authoritative consumer also requires GE-TRUST and actual authenticated baseline records. GE-RELEASE may use pinned native/adapter compatibility fixtures without waiting for final CG-ADAPTER production rollout or END-TO-END.

No circular barrier: GitGuard supplies read-only candidate identities before FlowGuard consumes them; its optional write executor is a later phase. FlowGuard does not need Git mutation authority to evaluate a gate. GuardEngine's limited GE-RELEASE excludes its later END-TO-END task, so dependent Guards do not block the artifact needed to implement themselves.

## Common invariants

- Six independent specialists own domain parsing/policy; GuardEngine owns generic Contract/Rule/Evidence, not stages, tests, Git operations or approval issuance.
- Current `guard.partme.ai/v1alpha1` stays strict and unchanged; `guard.integration/v1alpha1` is a separate draft requiring GE-CONTRACT freeze. Schema version, crate semver, policy revision and analyzer version are distinct.
- Before complete binding exists, failure is a transport diagnostic without an envelope. Bound error/cancelled attempts have null decision; valid partial evaluation remains BLOCK/INDETERMINATE.
- Engine-backed envelope decision equals its referenced report. External approval never rewrites a specialist REQUIRE_APPROVAL; FlowGuard may create its own independently scoped report.
- Native CodeGuard behavior remains authoritative for its current interfaces; new adapter semantics are opt-in and command-aware. No silent exit-code/report rewrite.
- Freeze obligations before running. Approval cannot fix missing coverage. Preserve exact candidate/base/group, requirement scope, baseline/policy/analyzer/coverage and current expiry/revocation binding.
- Read-only by default; candidate code cannot hold approval/signing/merge credentials. Optional side effects require independent controller authority and recovery tests.

## Task tracking, compatibility and validation

Each change's tasks.md maps its own requirements to concrete files/interfaces, fixtures and acceptance. Mark runtime work complete only with actual implementation and test evidence. Existing completed task states are historical facts and are not reset or copied as newly completed tasks. Legacy umbrella items may reference new detailed acceptance without creating duplicate implementation owners.

Use official `@fission-ai/openspec@1.14.1` for this planning review:

```sh
openspec validate <change-id> --strict --no-interactive --json
```

This validates structure; it does not compile Rust, execute scenario tests, authenticate approvals or certify production readiness. The old GuardEngine/ArchGuard changes use pre-delta spec formatting; CodeGuard's older large change has long-requirement warnings under this validator. Preserve and report these separately from each new change's result rather than silently suppressing diagnostics or checking tasks. Their format remediation can be a separately reviewed follow-up; it does not require rewriting implemented behavior here.

Before implementation, record concrete choices for canonicalization/profile schema, baseline authority provider, revocation freshness, storage/retention, resource limits, supported language/tool versions, sandbox platform and package publishing. Any unresolved choice blocks only its dependent gate; no tool/provider or policy engine is selected merely by listing it as a candidate.
