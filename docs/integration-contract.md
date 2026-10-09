# Shared Guard integration contract — draft 0.1

Status: **target design, not implemented**. This identical document is reviewed with all seven repositories. GuardEngine owns its future versioning; each Guard owns domain payload semantics. It does not change the strict, currently implemented `guard.partme.ai/v1alpha1` protocol. Product names are GuardEngine, SpecGuard, ArchGuard, CodeGuard, TestGuard, GitGuard and FlowGuard; the historical wire namespace stays unchanged until an explicit migration.

## 1. Responsibility and version boundaries

| Owner | Responsibility | Must not claim |
|---|---|---|
| GuardEngine Contract Engine | Validate supported contract/fact representations and version compatibility | Domain requirement approval or Git authorization |
| GuardEngine Rule Engine | Evaluate domain-neutral relations deterministically | Parse specifications/source/tests or own workflow stages |
| GuardEngine Evidence Engine | Normalize engine inputs, compute reports/digests and recompute consistency | A digest or `signed: false` authenticates its producer |
| Six independent Guards | Discover/parse domain inputs; declare coverage; own policy meaning, source attribution and domain diagnostics | Unknown domain evidence is complete because no violation was found |
| Trusted integration controller | Authenticate producer/approver; bind immutable candidates and policy; enforce access, retention and freshness | A local ALLOW or uploaded report grants merge/release rights |

Keep four versions distinct: package semver, engine `apiVersion`, integration envelope `apiVersion`, and policy `metadata.revision` plus content digest. An analyzer has its own identity/version. A policy revision string is not proof of immutable content.

Current engine objects reject unknown fields and accept only their exact engine protocol version. New envelope fields must never be injected into GuardFacts, GuardContract or GuardReport. Draft envelope readers should likewise reject unknown fields and unsupported versions. There is no implicit N/N-1 support: negotiate explicit capabilities, use a reviewed adapter with fixtures, or fail closed. Changes to verdict meaning, canonicalization, required coverage, or trust semantics require a new supported schema version and compatibility tests. Package upgrades must not silently rewrite archived reports.

## 2. Draft GuardRunEnvelope

Proposed external JSON envelope `apiVersion: guard.integration/v1alpha1`, `kind: GuardRunEnvelope`. This name/version is reserved for design review only; no current engine parser accepts this object. Fields below are a proposed required contract unless marked nullable/optional. Durable storage/API schemas and executable validators remain implementation work.

| Field | Target type and invariant |
|---|---|
| `apiVersion`, `kind` | Exact strings above |
| `runId` | Opaque unique attempt ID; retries receive new attempt IDs |
| `producer` | `{guard, version, analyzerId, analyzerVersion}`; identity must also be authenticated externally |
| `binding` | `{repoId, taskId, worktreeId, requirementIds, candidateOid, baseOid, mergeGroupId, sourceSnapshotDigest, baselineDigest}`; requirement IDs sorted/unique; mergeGroupId nullable outside queue; baselineDigest nullable only where policy declares no baseline |
| `runStatus` | `completed`, `error`, or `cancelled`; describes execution, not a policy verdict |
| `decision` | `ALLOW`, `BLOCK`, `REQUIRE_APPROVAL` for completed runs; null otherwise |
| `coverage` | `{status, requiredScopes, observedScopes, missingScopes}`; status `complete` or `partial`; required scopes frozen before running |
| `artifacts` | `{contract, facts, report, domain}`; each nullable reference is `{uri, digest, mediaType}`; domain is an array. References resolve within allowed evidence storage, never arbitrary executable URIs |
| `approvalRefs` | Array of opaque authenticated approval-record references; empty means none, never implicit approval |
| `diagnostics` | Array of `{code, message, retryable, source}`; source nullable; no secrets |
| `startedAt`, `finishedAt` | UTC RFC3339 timestamps; finishedAt >= startedAt; freshness uses trusted controller clock |
| `expiresAt` | Nullable UTC deadline; null means no time TTL, not exemption from input/revocation invalidation |

A GuardRunEnvelope is emitted only after the full invocation binding and producer profile have been resolved and validated. Pre-binding failures (invalid arguments, unknown repository, ambiguous task/candidate/base, or unreadable policy needed to freeze scope) use a separate transport diagnostic with failure exit status; they do not emit a GuardRunEnvelope, fabricate OIDs, or fill required fields with empty strings. The proposed transport diagnostic is not an engine GuardReport or a published schema; its exact format is deferred to each versioned CLI/API contract. Statements below about error/cancelled envelopes apply only to attempts whose required binding and frozen coverage already exist.

For an engine-backed completed run, contract/facts/report references are required and the envelope decision must equal GuardReport.decision. An error may retain input or diagnostic artifacts but has no successful report claim. CodeGuard's compatibility adapter may initially reference a native report under domain with engine references null; declare this capability explicitly. Consumers requiring engine verification must reject that weaker profile, not manufacture a GuardReport or downgrade the obligation. Any adapter that synthesizes facts must publish its mapping/version and validate scope separately.

`candidateOid` and `baseOid` are full immutable object IDs checked against the repository's object format and actual objects (do not hard-code SHA-1 length). Repo identity is a controller-assigned stable identifier, not a mutable origin URL alone. For local dirty inputs a candidate commit does not describe all bytes: sourceSnapshotDigest must cover analyzed inputs, and authoritative merge evidence requires a clean, frozen checkout. `worktreeId` disambiguates local work, not trust. `sourceSnapshotDigest` is a digest of the declared source scope, not automatically the whole repository. The current ArchGuard manifest digest remains a manifest-scope artifact.

## 3. Existing engine decisions and transport

| Condition | Engine report / result | Current GuardEngine and ArchGuard exit |
|---|---|---|
| Complete scope, no blocking/review relation | ALLOW | 0 |
| Enforced relation matched | BLOCK | 2 |
| Review relation matched, no stronger block | REQUIRE_APPROVAL | 3 |
| Partial facts, with diagnostic | INDETERMINATE evaluations, BLOCK | 2 |
| Invalid/unsupported input, runtime I/O error, verification mismatch | No valid decision report guaranteed | 4 |

BLOCK dominates REQUIRE_APPROVAL, which dominates ALLOW. `advise` matches remain visible with PASS; PASS does not mean no observation. Approval cannot repair missing coverage, a tool crash or malformed policy. `verify` checks equality with a recomputed report and returns its decision code: consistent BLOCK evidence still exits 2.

GuardEngine `evaluate` and ArchGuard `check` emit JSON to stdout **only without** `--report`; with `--report` they write that file. GuardEngine `verify` emits its consistency message to stderr and has no JSON success envelope. Runtime errors are currently plain stderr, not this draft envelope. Consumers must check the process result and report binding rather than infer success from stdout or the presence of an old file.

Future check commands should adopt the 0/2/3/4 mapping and separate JSON result output from stderr progress/errors; exact flags belong in each Guard's design. Cancellation produces a cancelled envelope where possible and never a successful policy decision; target adapters normalize cancellation to an execution failure (4), while signal termination may prevent any output. CodeGuard's established native commands, report schemas, hook semantics and exit codes remain unchanged until explicit migration. Its adapter must interpret command + flags + report, not blindly map a numeric 2 to BLOCK. Document actual native behavior in CodeGuard's technical design.

## 4. Rule, contract, baseline and approval responsibilities

A Guard emits observed domain facts and coverage. A protected contract chooses enforcement; a candidate cannot weaken it by editing its own policy copy. Freeze the required obligation set first so removing a rule/test/document cannot make a check disappear. Engine neutral operators are only the evaluated mechanism; language imports, requirements weakening, flaky-test classification and stage inheritance belong to their Guard.

A target approved baseline record binds repository/scope, immutable source revision and content digest, policy digest, approval record and effective period. Candidate edits do not retroactively change that baseline. Domain owners define what counts as weakening, drift, exception or inherited scope; the controller verifies baseline authority and authenticates approvers.

An approval record must bind issuer/role, purpose/action, scope, artifact or candidate digest, policy/baseline revision, issue/expiry times and revocation status. The exact provider/schema is unresolved. Reject ambiguous identity, scope widening, expiry, revocation and missing provider responses. A Markdown `accepted` marker or an agent-provided boolean is not authority. Engine REQUIRE_APPROVAL remains intact in immutable evidence; a separate controller records whether the specified action is authorized. Approvals do not rewrite a technical verdict or permit incomplete analysis.

## 5. State, invalidation and concurrency

Target attempt lifecycle:

```text
queued -> running -> completed | error | cancelled
completed -> eligible | stale (controller freshness assessment)
eligible -> stale on changed inputs, expiry or revocation
```

Eligibility is a separate derived controller state, not a fourth envelope runStatus or a mutation of archived results. Rechecking creates a new attempt. Error/cancelled/partial evidence never fulfills a required complete obligation. A previous successful attempt cannot substitute for a failed recheck when the required binding differs.

Cache/reuse key must bind repo, requirement set/task scope, candidate/base/merge group, source snapshot, baseline, protected contract digest, producer/analyzer versions and required coverage. Controller authorization additionally considers current approval state/expiry and current trust policy; cached technical evaluation cannot cache away revocation. Run IDs identify attempts; deduplication keys identify equivalent immutable work. Do not key results only by branch name, worktree path or requirement ID.

Two parallel requirements may share deterministic extraction only when input/config/coverage keys match. Keep independent obligation sets, run histories and approval scopes. A completion is attached to its original binding; compare-and-set on the current binding prevents late PASS from replacing a newer FAIL or candidate. Per-target short locks protect authorized side effects, not long analysis runs. Retry reads safely; after uncertain mutation outcome reconcile remote state before retrying.

Invalidation triggers include candidate/base/merge-group changes, analyzed bytes, baseline, protected policy, analyzer/version/configuration, scope/coverage, dependencies on other requirements, and approval expiry/revocation. Immutable artifacts remain audit records; only their eligibility changes. A trusted controller recomputes affected obligations and propagates invalidation along explicit dependency edges.

## 6. CLI, library and CI paths

Local CLI and library evaluations are advisory within their explicit scope. Library callers get typed engine errors and own process exit mapping. Domain libraries must expose read-only analysis separately from side effects. CI captures process status, artifact digest and exact input binding; a missing output, parser error, unsupported capability or nonzero execution error blocks required gates.

For protected merge queues: authenticate the event, resolve the exact synthetic merge-group candidate and base, freeze obligations from protected policy, execute analyzers in isolated clean checkouts, validate artifacts/coverage and provider approvals, then publish the required check against that candidate. When the queue rebases or changes membership, invalidate prior eligibility and rerun. PR-head-only evidence cannot satisfy a merge-group obligation. GitGuard handles Git/ref authorization and side-effect preconditions; FlowGuard coordinates workflow obligations; neither replaces specialist analyses. Branch protection/ruleset configuration and signing are future integrations, not guarantees supplied by these documentation changes.

## 7. Security, storage and audit

Separate untrusted worktrees from protected policies, approval issuers and signing credentials. Analysis gets read-only repository permissions; optional execution gets short-lived action-scoped credentials only from a trusted controller. Redact secrets in paths, diagnostics and artifacts; constrain path traversal, symlinks, output destinations, file sizes, parser depth, subprocess time and network egress. Never execute embedded document instructions or policy code by default. Integrity hashes do not encrypt sensitive source.

Audit records should append actor/verified identity, action/purpose, binding, artifact digests, policy and approval references, outcome, causal predecessor and controller timestamp. Set retention/access/deletion policies per tenant; store only necessary source excerpts. Signing/attestation keys, provider selection, event-store persistence and retention duration remain decisions before implementation, not reasons to withhold this document review.

## 8. Verification and staged acceptance

1. Freeze schemas and mapping fixtures: valid, unknown version/field, malformed, missing artifact, wrong digest, missing coverage and unsupported capability. Do not claim the draft implemented until executable consumers pass.
2. Engine adapters: golden vectors for normalization and decision precedence; CodeGuard native/adapter parity with explicit command semantics. No loss of native functionality.
3. Trust/freshness: forged approval, expired/revoked approval, changed baseline/policy/analyzer, source drift and tampered evidence all fail required gates.
4. Concurrency/queue: two requirement runs cannot cross-satisfy; delayed old completion cannot overwrite; new queue candidate forces recheck; duplicate mutation is reconciled safely.
5. Rollout: advisory capture, shadow comparison, opt-in protected checks, then explicit enforcement; rollback the adapter/controller independently while keeping immutable evidence and unchanged native commands.

Evidence required per phase: reviewed schema/mapping, fixture inputs, recorded actual output/exit status and authenticated CI binding where applicable. Documentation review, checked OpenSpec boxes and test-file existence are not execution evidence.
