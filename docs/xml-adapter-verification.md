# XML adapter candidate verification

This local branch pairs the unchanged Rust production source with the GitGuard plugin's XML-native branch. The adapter imports `gitflow.policy.load` and delegates gate checks; it contains no independent JSON fallback. Both repositories use local branch `feat/xml-native-guard-20261030`; exact tested commits are recorded in the delivery report. Nothing was pushed or released.

The prior Python fixture wrote only `.gitflow/state/activation.json`. With the XML plugin, four of five tests failed honestly with `policy_missing`. The fixture now writes XML using the actual adapter encoder; negative legacy JSON files deliberately prove they cannot become runtime authority. Missing and corrupt XML remain unverified, changed XML takes effect, and linked worktrees read the common policy.

Validation uses a real locally built Linux binary, not a mocked executor:

```sh
cargo build --locked --offline
GITGUARD_BIN=/absolute/path/to/built/gitguard GITGUARD_ADAPTER_ROOT=/absolute/path/to/xml-plugin python3 tests/policy_check.py -v
cargo test --locked --offline
```

Seven Python integration tests passed, independently reproduced: CLI/MCP negative-fix-recheck, author environment, protected branch and missing source, no executable-filter execution, XML-only/missing/corrupt/changed policy, shared worktree policy, and eight-profile context subtests. `cargo test` succeeds with **zero Rust tests** and is not counted as behavioral coverage. Dependency checkout was GuardEngine `fe36a46b1711d84d21e6bea75358bb6b55c579ce`; no dependency or source changes were made.

Before any future candidate CI, obtain push authorization and pair the new plugin SHA. The existing workflow still pins old plugin `89e7c8d2`; it does not validate this pair. GuardEngine's shared installation verifier still seeds JSON/JSON-drift fixtures and needs a coordinated fixture update before claiming that whole workflow passes with XML. That separate verifier was not modified or reported passing. Windows/macOS, actual host invocation, hook trust and formal releases remain outside this local Linux result.
