# XML adapter and candidate pairing verification

GitGuard's policy evaluator still delegates to the explicitly selected plugin's XML loader. CLI/MCP policy checks consume the complete validated policy; missing or corrupt XML never falls back to legacy JSON.

The exported `context` domain now follows `reference_schema_version: 1`: status, profile, revision, SHA-256 and `policy_location`. It deliberately omits `roles`, remote mappings, merge methods and message patterns so the generic installation renderer does not copy business rules into AGENTS. Consumers needing a verdict must call `policy check`; reference metadata is not a verdict. CLI and MCP use the same reference projection. Installation doctor still detects stale references and requires explicit `instructions sync` after XML changes.

Both candidate repositories use local branch `feat/xml-native-guard-20261030`. The engine workflow's `plugin_sha` input accepts only a full lowercase40 commit; its fixed fallback is the paired local plugin commit recorded in the workflow. Checkout identities are verified against that value, the workflow engine SHA and pinned GuardEngine `fe36a46b1711d84d21e6bea75358bb6b55c579ce`. No workflow was remotely executed here.

Local actual-binary evidence: seven CLI/MCP/context tests pass (including all eight profiles), eight install/init responsibility tests pass, and the shared installation wrapper runs all ten original method names plus one install/init responsibility test: ten pass and one unchanged six-tool joint test skips because only GitGuard is selected. Portable package structure and actual stdio discovery pass. Cargo builds offline; cargo test currently contains zero Rust unit tests, so it is not counted as behavior coverage.

The shared GuardEngine verifier remains unchanged. `tests/run_installation_xml.py` imports its entire pinned suite and overrides only the obsolete JSON fixture; it checks the exact inherited method set. The new workflow calls this adapter instead of the old JSON fixture directly. Same-project init/install now succeeds because install alone owns guidance and init leaves AGENTS untouched. See [the ownership contract](xml-installation-fixture.md).

Windows/macOS current-candidate execution and actual Agent-host invocation remain unverified. The plugin business CI `check` remains unconfigured until independently trusted XML distribution is defined; passing this engineering candidate workflow does not approve a PR under its own XML.
