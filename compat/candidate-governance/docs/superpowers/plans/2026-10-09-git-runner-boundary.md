# Git runner boundary implementation plan

> Execution: same owner implements; root assigns independent review after commit. Explicit parent authorization overrides an extra plan approval pause.

**Goal:** Complete task1.2 only when real Git subprocess behavior meets its original bounded observation requirement.
**Architecture:** Keep pinned /usr/bin/git and private copied object store. On Linux x86_64/aarch64, prevent child process creation/network sockets with an audit-architecture-checked seccomp filter installed after no_new_privs in pre_exec. Replace reader threads/unbounded joins with nonblocking bounded pipe supervision; direct Git process is the entire permitted process tree. Unsupported platforms fail explicitly. This is a narrow local process profile, not an OS filesystem sandbox.
**Spec:** openspec/changes/add-candidate-bound-git-governance/specs/git-candidate-preflight/spec.md; task1.2.

## Constraints and investigation

Existing env_clear, explicit config --no-includes parsing, private generated config, alternate rejection, NUL diff parsing and diagnostic enums already avoid most repository helper/config leakage. Existing runner only kills a process group, joins reader threads without a cleanup deadline, accepts unbounded requested budgets, and does not deny child creation. Neither process groups nor no_new_privs alone are complete confinement. Public APIs never accept arbitrary executables or Git argv.

## Review focus

- Existing source config/environment cannot execute helpers, textconv, hooks or hidden fetch; real malicious repository fixtures leave sentinel and full repository state unchanged.
- Git cannot create descendant processes or network sockets even if an internal future command reaches an execution entry; actual Git alias probe must fail before marker creation.
- Output flood and timeout cannot hang on reader joins; captures share one bounded budget, cleanup is bounded and failure is explicit.
- Linux audit architecture/x32 cannot bypass syscall denial; filter installation failure must prevent exec.
- All existing Git candidate/merge-tree/object/diff commands still pass real fixtures; a denied unsupported Git command is an error, never complete coverage.

## Steps

- [x] Add runner unit REDs for real Git helper creation and above-profile limits, and regression cases for native output/time behavior. Record behavioral failures before implementation.
- [x] Add src/git/process.rs with checked pre_exec filter, nonblocking capture, deadline and bounded direct-child cleanup; runner retains fixed command/env construction. Tests use fixed private fixtures only, no public shell/test execution API.
- [x] Add tests/runner_security.rs repository fixtures for helper/diff/textconv/hook/env/alternates, token redaction and unchanged refs/index/worktree; retain NUL paths and merge preview regression suite.
- [x] Run focused RED/GREEN, full cargo test, strict clippy/fmt and document supported architecture, limits and remaining sandbox limitations. Keep task checkbox unchanged for independent review; commit locally only.

Follow-up review owns task acceptance. Native fsck RED identified optional accelerator subprocesses; disabling those private optimizations preserves full loose/packed object verification. Further behavioral REDs prove swallowed syscall denial and core-dump hard-limit gaps; both are corrected.
