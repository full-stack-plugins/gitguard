# All-branch integration, 2026-10-30

The current default runtime remains the verified XML-based three-platform installer/context/policy CLI. The plugin main is fixed at `009663e421e450f07ead29c32cc71ff238174e1b`; its two former branches are contained. No releases, tags, branch deletion, trust changes or protection bypass are part of integration.

Engine inputs:
- `feat/xml-native-guard-20261030`: `09fccb3903f5796c26140244a4fa39562e66c9fa`, already contains `fix/local-cli-readiness-20261028` (`374866b8b37b933e369ff2b19a70e6605a5c3919`).
- `impl/guard-roadmap-20261009`: `6be082a4b1ae91fce1075ba4fadcb7c17bbc21bc`, already contains `docs/guard-design-20261009` (`5c511ccbd7c6d11c01dab5d80deff0a4f15383dd`).

The old implementation cannot simply overwrite Cargo.toml/src/main.rs: it imports Unix APIs and GuardEngine `integration` APIs absent from the current dependency. Its entire tree is retained under `compat/candidate-governance` with explicit Linux-only build/verification and the original API dependency pin. Its own source, tests, fixtures, examples, design and former workflow definitions remain inspectable. Compatibility dependency location, explicit private test-fixture permissions and deterministic fixture stdin delivery are adapted; this is not an unreviewed port or reimplementation of the shared engine.

Default Cargo.toml/lock, CLI and installation wiring keep the XML candidate behavior. The legacy compatibility target is not a workspace default member, not registered as a host tool, and not installed by the current installer. Its opt-in historical JSON observation adapter cannot become current GitFlow rule authority. Privileged writes remain off by default, and compatibility CI operates only on disposable repositories. The archived workflow is inactive under the compatibility subtree; root `candidate-compatibility.yml` explicitly executes the preserved test suite with its separate dependency checkout. Default three-platform workflow continues to use the current `guard-install` dependency.

Conflicted README/design material is preserved in the compatibility source tree while root documentation states the current contract. Merge ancestry preserves both branch histories. Integration success does not imply real Agent-host acceptance, trusted business-CI XML distribution or a production release.

## Local integration verification

- Compatibility default: 146 passed, 0 failed, 1 pre-existing ignored, 0 filtered out.
- Compatibility enabled: 163 passed, 0 failed, 1 pre-existing ignored, 0 filtered out.
- The ignored five-Guard integration test requires separately selected GitGuard/FlowGuard binaries; it was not enabled or claimed verified here.
- Original Clippy safety targets and isolated local_bare_apply example passed. Default runtime installation (10 pass/1 existing joint-tool skip), policy7, guidance8 and package/stdio checks passed.
- Independent archive comparison verified all 589 original files and executable bits at import. Subsequent native CI required explicit 0700 permissions in five legacy fixture files (test sections only). A sixth fixture file, `tests/cli_outcomes.rs`, now delivers prepared input through a private temporary regular file to avoid BrokenPipe when the child rejects argv before reading stdin; a deterministic one-MiB invalid-argv regression was added. All original CLI exit/stdout/old-report assertions remain byte-identical; no production algorithm or assertion was removed. Default Rust source/Cargo/lock match the XML candidate.
- Remote integration CI must pass on the exact merge commit before main update.

### Full-suite fixture correction
The first compatibility CI run exposed seven unsafe fixture roots under runner umask 022. A local umask-022 run reproduced the same seven failures. Builders used by private-store tests now explicitly request 0700, independent of the host umask; deliberate unsafe 0755 cases and production rejection checks remain unchanged. The workflow retains the historical test-process umask 022. Full default/privileged validation under umask 022 and broader-umask private-store checks must pass before main integration; failures are retained in the external integration evidence.

A diagnostic full-library run under umask 0002 also exercised Git-created group-writable repository children and correctly encountered UnsafeTarget. It is retained as a failed diagnostic, not a passing acceptance run. The CI acceptance profile remains the historical umask 022; private evidence/intent-store tests are additionally checked under 0002 without weakening target permission validation.

The second compatibility CI run exposed a parent-to-child stdin pipe race on early argument rejection. The new large-input regression reproduced BrokenPipe before the fixture helper changed; input serialization and rewind now complete before child launch, with no ignored I/O errors. The repaired full default and privileged suites under umask 022 passed 146 and 163 tests respectively, each retaining one pre-existing ignored test; the original required Clippy target set passed. These are local verification results pending the next remote CI run.
