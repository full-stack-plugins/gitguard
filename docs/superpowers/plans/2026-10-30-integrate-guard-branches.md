# GitGuard Branch Integration Plan

> **For agentic workers:** Apply independent implementation/review checks; do not merge unresolved semantic conflicts.

**Goal:** Integrate all authorized GitGuard and plugin branches without losing effective history or reverting the current XML contract.
**Architecture:** Use isolated integration branches. Preserve ancestry with fast-forward/merge commits, exact candidate pins and normal protection enforcement.
**Tech Stack:** Git, Python standard library, Rust, existing GitHub candidate CI.
**Spec:** Latest user contract and plugin openspec/changes/xml-native-guard/specs/xml-native-guard/spec.md.

## Global Constraints
- install owns AGENTS; init only initializes XML; XML is the sole GitFlow policy authority.
- Preserve old effective capabilities; no force push, branch deletion, tag/release or security changes.
- Validate exact integrated commits on three native platforms; protocol replay is not host acceptance.

## Review Focus
- Divergent old Rust CLI and missing old GuardEngine APIs cannot be silently dropped or substituted.
- Old JSON adapters must not become a second runtime policy authority.
- Old privileged executors remain disabled by default; no real-repository writes during tests.
- Integration workflow must not publish/deploy unexpectedly.
- Remote concurrent updates require re-evaluation, never overwrite.

## Tasks
- [x] Fetch remote refs and inspect all branch containment, main protection and workflows.
- [x] Plugin isolated integration fast-forward to previously verified exact SHA; independent review.
- [x] Push plugin main without force; verify remote ancestry and same-SHA main checks.
- [x] Engine isolated integration includes current XML and readiness branches; trial merge old implementation.
- [x] Resolve build/CLI/documentation conflicts while preserving current XML semantics. If external dependency/functional decision is required, report precise blocker before changing scope.
- [ ] Run complete relevant tests, independent review and three-platform candidate CI against paired plugin SHA.
- [ ] Integrate safe engine result to main under protection rules; verify every branch and final checks. No false all-merged claim if any branch remains blocked.
