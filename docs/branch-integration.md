# All-branch integration, 2026-10-30

The current default runtime remains the verified XML-based three-platform installer/context/policy CLI. The plugin main is fixed at `009663e421e450f07ead29c32cc71ff238174e1b`; its two former branches are contained. No releases, tags, branch deletion, trust changes or protection bypass are part of integration.

Engine inputs:
- `feat/xml-native-guard-20261030`: `09fccb3903f5796c26140244a4fa39562e66c9fa`, already contains `fix/local-cli-readiness-20261028` (`374866b8b37b933e369ff2b19a70e6605a5c3919`).
- `impl/guard-roadmap-20261009`: `6be082a4b1ae91fce1075ba4fadcb7c17bbc21bc`, already contains `docs/guard-design-20261009` (`5c511ccbd7c6d11c01dab5d80deff0a4f15383dd`).

The old implementation cannot simply overwrite Cargo.toml/src/main.rs: it imports Unix APIs and GuardEngine `integration` APIs absent from the current dependency. Its entire tree is retained under `compat/candidate-governance` with explicit Linux-only build/verification and the original API dependency pin. Its own source, tests, fixtures, examples, design and former workflow definitions remain inspectable. Only compatibility location assumptions are adapted; this is not an unreviewed port or reimplementation of the shared engine.

Default Cargo.toml/lock, CLI and installation wiring keep the XML candidate behavior. The legacy compatibility target is not a workspace default member, not registered as a host tool, and not installed by the current installer. Its opt-in historical JSON observation adapter cannot become current GitFlow rule authority. Privileged writes remain off by default, and compatibility CI operates only on disposable repositories. The archived workflow is inactive under the compatibility subtree; root `candidate-compatibility.yml` explicitly executes the preserved test suite with its separate dependency checkout. Default three-platform workflow continues to use the current `guard-install` dependency.

Conflicted README/design material is preserved in the compatibility source tree while root documentation states the current contract. Merge ancestry preserves both branch histories. Integration success does not imply real Agent-host acceptance, trusted business-CI XML distribution or a production release.

## Local integration verification

- Compatibility default: 145 passed, 0 failed, 1 pre-existing ignored, 0 filtered out.
- Compatibility enabled: 162 passed, 0 failed, 1 pre-existing ignored, 0 filtered out.
- The ignored five-Guard integration test requires separately selected GitGuard/FlowGuard binaries; it was not enabled or claimed verified here.
- Original Clippy safety targets and isolated local_bare_apply example passed. Default runtime installation (10 pass/1 existing joint-tool skip), policy7, guidance8 and package/stdio checks passed.
- Independent archive comparison verified all 589 original files and executable bits; only the documented dependency path differs. Default Rust source/Cargo/lock match the XML candidate.
- Remote integration CI must pass on the exact merge commit before main update.
