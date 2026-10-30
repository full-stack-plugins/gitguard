# GitGuard

[English](README.md) · [简体中文](README.zh-CN.md)

GitGuard guards branch policy around the project's `git-common-dir/gitflow.xml`. It is a local assistant to native Git, not a replacement Git client or an unbypassable remote gate.

## Current runtime

The default Rust executable provides project installation, managed AGENTS guidance, read-only context, policy checks and MCP. Its explicitly selected [GitGuard plugin](https://github.com/full-stack-plugins/gitguard-plugin) provides profile initialization and native/Agent guard integration.

- Project `install` owns the receipt-protected AGENTS block and MCP configuration. Unknown or modified blocks are not overwritten.
- Plugin `init` selects one of eight GitFlow profiles and initializes XML only. It does not read or modify AGENTS. Linked worktrees share policy through git-common-dir.
- Policy checks use XML only; missing or corrupt XML is unverified, with no legacy JSON fallback.
- Initializing configuration does not activate hooks. Existing hooks/managers are preserved; explicit integration is required. Native hooks cannot intercept every manual fast-forward/no-commit/no-verify operation.
- Actual Agent host loading and trusted business-CI XML distribution remain unverified/unconfigured. Installation, protocol replay and candidate CI do not prove production readiness.

## Build and use

Check out the pinned GuardEngine beside this repository; see `.github/workflows/agent-installation.yml` for the exact dependency and paired plugin SHA. Rust 1.85+ is required for this default target.

```sh
cargo build --locked
./target/debug/gitguard --help
./target/debug/gitguard install --agents codex --project /absolute/project --gitflow-root /absolute/gitguard-plugin --python /absolute/python --dry-run
python /absolute/gitguard-plugin/scripts/gitflow.py init /absolute/project --profile github-flow --apply --json
./target/debug/gitguard policy check --project /absolute/project --gitflow-root /absolute/gitguard-plugin --python /absolute/python --action merge --source feature/example --target main
```

Use `.exe` on Windows. Remove installer `--dry-run` only for an intended project installation. The plugin launcher bootstrap `install --bin-dir` is distinct from Rust project installation. `allow` is only a policy verdict, not technical approval or permission to write Git.

Other runtime commands: `uninstall`, `context`, `instructions render|sync`, `doctor --integration`, `serve --mcp`. Ordinary native Git operations remain the user workflow.

## Preserved candidate-governance implementation

The older roadmap branch is retained in [compat/candidate-governance](compat/candidate-governance/COMPATIBILITY.md), including its library, CLI, tests, fixtures, documentation and examples. It is a separately built Linux compatibility target with its original pinned GuardEngine API dependency and Rust 1.90 minimum. It is not installed or called by the default runtime. Its historical JSON observation adapter is not current policy authority; privileged execution remains disabled by default. The compatibility workflow runs the preserved tests explicitly. Its optional local reftable receipt capability requires Git 2.47.3 or newer plus an actual private-repository capability probe; newer stable distribution builds are not blocked by an exact-version pin. This optional minimum does not constrain the default runtime or generic files-backend behavior.

See [branch integration decisions](docs/branch-integration.md), [installation ownership](docs/xml-installation-fixture.md), [paired validation](docs/xml-adapter-verification.md), and [plugin trust boundaries](plugin/README.md).
