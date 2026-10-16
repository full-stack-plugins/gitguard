# GitGuard professional policy checks verification

Date: 2026-10-16. Scope: the existing GitGuard CLI/MCP integration, governed by `guardengine/openspec/changes/professional-guard-checks`. Brownfield increment; no local SDD initialization, branch changes, commits, global installation or public crate edits.

## Implemented

- `policy check --action branch|merge|commit`, declared in `domain_commands`.
- `gitguard_policy_check`, backed by the same Rust validation and adapter invocation as CLI.
- Original GitFlow `policy.load`, `role_for`, `service.gate` remain the policy authorities. No duplicate branch profile, regex engine, merge matrix or commit rule implementation.
- Branch naming checks a proposed name; merge checks existing local source/target roles; commit checks the current branch and proposed message/identity rules.
- No Git writes. Missing activation, drift, unknown local refs and unavailable adapters cannot produce `allow`. Technical quality remains `not_evaluated`, including allowed policy checks.

## Executed evidence

Local environment: Windows PowerShell, existing Rust/Cargo and bundled Python. `CARGO_HOME=E:/workspaces/workspace-agent-skills/.guard-build/cargo`, `CARGO_TARGET_DIR=E:/workspaces/workspace-agent-skills/.guard-build/targets/gitguard-agent`, `TEMP`/`TMP` use `E:/workspaces/workspace-agent-skills/.guard-build/tmp`.

1. RED: wrote `tests/policy_check.py` before implementing the command. Both initial tests failed with CLI exit 4, `unsupported command; use --help for implemented capabilities`.
2. `cargo fmt --check`: passed.
3. `cargo build --locked`: passed.
4. `cargo clippy --locked -- -D warnings`: passed after replacing `args.get(0)` with `args.first()`.
5. `GITGUARD_BIN=<target>/debug/gitguard.exe python -B tests/policy_check.py`: **3 tests passed, 99.519s**. Coverage includes six CLI/MCP result comparisons across denied/corrected branch, merge and message requests; protected branch commit denial; unknown local ref; invalid Git branch name; missing trusted adapter; missing activation; shared candidate drift; unsupported write argument rejection; unchanged fixture Git status.
6. `git diff --check`: passed.
7. `python -B ../guardengine/scripts/verify_guard_installation.py --bin-dir <target>/debug --tools gitguard --gitflow-root ../gitflow-plugin`: **8 passed, 1 failed, 1 skipped** (77.145s). The failure is the shared MCP inventory assertion omitting the new `gitguard_policy_check` tool; reported to the coordinating agent, who owns the shared verifier. The six-tool combined test skips because only GitGuard was selected. Existing installation/coexistence, conflict rollback, capability truthfulness, GitFlow drift/refresh, linked-config rejection, binary lifecycle/ownership and untrusted-receipt tests passed.

## Limits

This verifies local policy checks, not protected-host integration, remote CI execution, historical commit scanning, final merge candidate freshness or Git writes. The trusted Python adapter is executable code selected explicitly by the caller. CI was updated to run the professional fixture test on its existing Linux/Windows matrix, but remote execution was not observed here. The shared installation inventory regression remains pending the coordinator's update and rerun.

## Cross-review P1/P2 regression repair

The review reproduced two defects before repair: (1) a configured clean filter created a marker during a supposedly read-only branch check, which still returned allow; (2) Windows removed GIT_AUTHOR_EMAIL and GF102 checked the configured default email, allowing the forbidden actual author.

- Added both real-process regressions first: **2 failed** for the expected marker execution / incorrect allow reasons.
- Shared Windows/POSIX Git transport now queries effective clean/process filter configuration before any status. Nonempty commands return `executable_filter_unverified` without running status/filter. Empty local overrides are respected; test fixtures disable inherited system LFS commands only in their own temporary repositories.
- Windows removes repository/config routing overrides but preserves native author/committer identity environment. POSIX retains its original GitFlow transport and receives the same filter preflight; optional index writes are disabled on both platforms. Original GitFlow policy code is unchanged.
- `cargo build --locked --offline`: passed.
- `GITGUARD_BIN=E:/workspaces/workspace-agent-skills/.guard-build/targets/gitguard-agent/debug/gitguard.exe python -B tests/policy_check.py`: **5 tests passed, 35.562s**, including clean/process CLI+MCP no-execution checks and forbidden-author CLI+MCP denial.
- `cargo fmt --all -- --check`, `cargo clippy --locked --offline --all-targets -- -D warnings`, `git diff --check`: passed.
- `cargo test --locked --offline` completed with **0 Rust unit tests**; the five Python process tests above are the behavioral evidence.

Real execution was observed on Windows only; the common preflight is used by POSIX, and existing CI contains both platforms, but Linux CI execution was not observed. Effective inherited LFS clean/process commands intentionally make checks unverified. Concurrent repository/config mutation still requires a trusted isolation boundary; this preflight does not claim to be a sandbox. No global Git configuration, gitflow-plugin or other Guard source was changed by this repair.
