# Frozen local integration capability mapping

This consumer maps to **guard.integration/v1alpha1**. The engine's original `guard.partme.ai/v1alpha1` objects remain unchanged. The mapping is a local review artifact; no remote publication or production activation is implied.

- GuardEngine repository commit: `e84b30676936b73e7403efebb2831688d566fb46`.
- Schema: `schemas/integration/v1alpha1/guard-run-envelope.schema.json`; SHA-256 `7eb99f016d19751b419be83e5209f9def16bde898be69b77f77d6af2d2f1bc41`.
- Capability manifest: `schemas/integration/consumer-matrix.json`; SHA-256 `8efcbc3bc0bd4dc2ff70998fe07cee53832385c48b7d67564e19b0d8d0f05cd3`.
- Frozen bytes: `tests/fixtures/consumer-corpus/`; executable verification: `cargo test --locked --offline --test consumer_corpus` in that GuardEngine checkout.
- Consumer positive cases: `gitguard-local`. Individual source pins, raw-byte digests and declared coverage are recorded in the manifest; crate version alone is insufficient.

Actual SDK over a real temporary Git candidate and frozen local policy fixture. No authenticated hosted merge controller or required-check protection claim.

## Required positive and negative mapping

Engine-backed completed evidence requires exact contract/facts/report references, successful raw-byte digest verification and recomputation, and matching producer/analyzer, source snapshot, technical outcome and declared coverage. Partial analysis remains BLOCK. Bound error/cancelled runs have null decisions and retain diagnostics; unresolved pre-binding input has no fabricated envelope.

Unknown schema versions and fields, malformed references, altered artifact bytes, contradictory report/envelope results and missing engine references are rejected. Native-only evidence cannot satisfy an engine-backed requirement. No producer becomes qualified because a wire version matches; missing required observations remain missing. The corpus includes explicit positive and negative cases for these rules.

The domain Guard owns scope, native command mapping, analysis and facts. GuardEngine owns generic rule evaluation, envelope validation and evidence consistency. External controllers own current candidate/baseline binding, authenticated authority, approvals and effects. Local immutable history or byte integrity cannot substitute for authenticated authority or fresh eligibility.

## Explicit gate limits

The named local protocol mappings have no unresolved field/version default. Any unlisted producer/profile pair, breaking field/semantic change, absent required producer, or stronger unsupported capability blocks that combination pending an explicit versioned review. Production identity, authenticated approval/revocation, candidate/source equivalence where unverified, hosted enforcement, public release and N/N-1 compatibility are not established by this mapping. Existing native commands and reports must remain independently compatible.

See the active [consumer change](../openspec/changes/add-candidate-bound-git-governance/design.md). Completion checkboxes require independent evidence for their full scope; this document alone completes no runtime task.
