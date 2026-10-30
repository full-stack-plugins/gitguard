# Preserved candidate-governance implementation

This directory preserves the complete tree from GitGuard branch `origin/impl/guard-roadmap-20261009`, commit **`6be082a4b1ae91fce1075ba4fadcb7c17bbc21bc`**. Source, tests, examples, fixtures, schemas, documentation, original README files and the historical workflow remain together. It is a separate compatibility package, not the installed or default GitGuard CLI. The current repository-root implementation and its XML contract take precedence.

## Build boundary

The compatibility package requires **Linux**, Rust **1.90+**, and the original GuardEngine root API at **`b0e4b445e1a20a9354d55ebb41bbedaf16ec8a74`**. Its `Cargo.toml` points to `../../../guardengine-compat`, a sibling of the enclosing GitGuard checkout. That dependency must be a separate checkout of the exact revision; the current installer dependency is not a substitute. The older root API exposes `guardengine::integration`, including eligibility, attempt-store and evidence-envelope types absent from the newer shared root API.

From the enclosing GitGuard checkout, after preparing the pinned sibling dependency:

```sh
cargo test --locked --manifest-path compat/candidate-governance/Cargo.toml
cargo clippy --locked --manifest-path compat/candidate-governance/Cargo.toml --all-targets -- -D warnings
```

These are verification instructions, not claims that every target has passed. The package has Unix-specific filesystem/process code, fixed `/usr/bin/git` and `/dev/null` paths, and a restricted Linux process profile. Windows/macOS compatibility is not implied by the current root CLI's platform support. Build outputs and command invocation are separate from the current root package; do not install this historical binary over the current `gitguard` executable.

## Policy and execution boundaries

The current runtime authority remains `<git-common-dir>/gitflow.xml` through the current shared policy loader. The preserved `adapters/gitflow` code and JSON captures are an explicitly opt-in historical observation profile. They must not supply current runtime policy, a JSON fallback, CI authorization or a replacement for the latest XML gate. Legacy generic evidence contracts and request JSON are their original compatibility interfaces, not additional GitFlow policy authorities.

Privileged writes remain **off by default**: Cargo's default feature set is empty, `ExecutionConfig.enabled` defaults false, and the actual local apply functions require both `privileged-execution` and enabled configuration plus their original authorization/state checks. The generic operation entrypoint still refuses authorization. Retaining this source does not enable mutation transports, credentials, remote push, host trust or production enforcement. Privileged fixture examples must only be run under separately authorized, isolated acceptance procedures.

The archived `.github/workflows/` under this directory is historical source; GitHub does not execute nested workflow directories. Any current compatibility CI must explicitly select this manifest, fixed dependency and Linux profile. Fixture provenance, captured output, script source pins and source-bundle hashes remain historical evidence, not refreshed results.

## Preservation record

The export contains all **589 original regular files**, without `.git` metadata or symlink extraction. The only original-file edit is the compatibility Cargo dependency path from `../guardengine` to `../../../guardengine-compat`; this document is added. No original source, fixture, assertion or README was removed or rewritten, and the original `Cargo.lock` is retained.

Internal Rust fixture paths resolve relative to this package's own `CARGO_MANIFEST_DIR` or source file and remain valid inside the exported tree. `scripts/prepare-local-compatibility.py` deliberately archives its original historical `SOURCE_PIN`; its old manifest transform applies to that archived source bundle, not this adjusted compatibility manifest. It therefore remains unchanged. Capture scripts and provenance files retain their original source/environment pins and must not be presented as newly executed captures.
