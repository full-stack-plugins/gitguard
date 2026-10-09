# GitGuard Candidate-bound Governance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. 本次仅规划；所有 checkbox 为未来工作，不执行实现，不将文档校验算作产品完成。

**Goal:** 先交付独立只读 GG-CANDIDATE，再逐步接入可信领域证据与默认关闭的可选 Git 写执行器。

**Architecture:** GitGuard 保留 Git 观察、范围和对象语义；GuardEngine 提供通用协议验证/求值/证据，可信控制面负责身份、批准与 grant。GG-CANDIDATE 先于 FlowGuard gate；可选执行后于外部授权审查，不形成整仓库循环依赖。

**Tech Stack:** 候选 Rust 2024、Serde、Clap、受限 Git CLI；库版本、异步运行时、最低 Git 能力和持久存储待所属任务验证后决定，目前没有安装或实现这些依赖。

**Spec:** [候选](specs/git-candidate-preflight/spec.md)、[证据](specs/git-evidence-integration/spec.md)、[可选执行](specs/git-controlled-execution/spec.md)、[增量设计](design.md)。依据：[架构](../../../docs/architecture.md)、[技术方案](../../../docs/technical-design.md)、[集成契约](../../../docs/integration-contract.md)、[路线图](../../guard-roadmap.md)。

## Global Constraints

- 基线 main `e03b5fd06d8b3d81d4bbbd11ff0fb70bea00d485` 与规划前 HEAD `54bff26895dbd91875b8ff4af035fd4fcb6de509` 无源码/测试/构建清单/旧 OpenSpec；以下源码路径全为未来创建，不覆盖旧 change。
- `guard.partme.ai/v1alpha1` 严格字段/精确 `forbid_relation` 不变；`guard.integration/v1alpha1` 是独立草案，版本/capability 不支持必须拒绝，无隐含 N/N-1。
- 绑定/producer/coverage 冻结前仅诊断；冻结后 error/cancelled decision=null；engine-backed completed decision=report.decision；partial valid facts=BLOCK/INDETERMINATE。
- 专家 REQUIRE_APPROVAL 不由批准改写，partial/error 不能被批准修复；FlowGuard 自己的 gate 报告与外部 Git 授权分离。
- 组1–2独立产出 GG-CANDIDATE；组3本域投影等 GE-CONTRACT/ADAPTER，可信消费另等 GE-TRUST 与按需领域证据；组4可选，不能阻塞前面的只读阶段；独立生产分发等 GE-RELEASE。
- 每项实现先建立命名失败 fixture/assertion 再完成最小实现并运行相应测试；验收保存真实命令/输出/exit/artifact，而不是勾选文字。不得在用户分支 force/reset/delete，不改变 CodeGuard 或外部插件原生行为。

## Review Focus

- 非 UTF-8 路径、大小写、symlink 与 rename 两端不能逃逸范围（1.4、1.5）。
- repo config/alternate/helper 能绕过朴素 argv 安全；资源耗尽与 secrets 日志同样需拒绝（1.2）。
- tree 相同但父关系不同不是同候选；dirty/index 不能冒充 commit（1.3、2.1、2.2）。
- 迟到完成、取消与目录复用不得污染另一需求或当前结果（2.3、3.4）。
- 远端写后断连且目标继续前进，不可仅以当前 ref 判断是否执行（4.4、4.5）。

## 1. Read-only foundations — G0；无 FlowGuard 或写端依赖

**Interfaces:** `discover(root) -> RepositoryRef`；`resolve_subject(repo, request) -> FrozenSubject`；`scan_scope(subject, TaskScope) -> ChangeSet`。类型字段遵循 design 第3节；异常用 `PreflightDiagnostic`，不伪造 envelope。文件均待创建。

- [ ] 1.1 在 `src/git/repository.rs`、`src/subject.rs` 与随切片引入的 `Cargo.toml` 实现稳定 repo/root/object-format 发现，固定 Rust/依赖版本并记录能力 ADR；`tests/repository.rs` 覆盖坏根、common Git dir、SHA-1/SHA-256 和未知算法，确认不以 origin URL 认证身份。（Requirement: Immutable repository and subject binding）
- [ ] 1.2 在 `src/git/runner.rs` 冻结 argv、环境/config 隔离、NUL 输出、时间/字节/进程树限制；`tests/runner_security.rs` 用恶意 helper/diff/hook/alternate、超时及令牌诊断 fixtures，断言无执行入口、无隐藏 fetch、无泄密及无 refs/index/worktree 修改。（Requirement: Bounded Git observation and semantic uncertainty）
- [ ] 1.3 在 `src/subject.rs` 实现 `FrozenSubject` 的 worktree/index/commit/tree-preview 区分、OID 实物校验和 snapshot digest；`tests/subjects.rs` 断言 dirty HEAD 不得充当权威候选、missing object 不生成 envelope、ref 运行中变化不被重新解释。（Requirement: Immutable repository and subject binding）
- [ ] 1.4 在 `src/scope.rs` 与 `schemas/task-scope.schema.json` 冻结受保护 TaskScope、基线 immutable ref/digest 及 advisory 信任标记；`tests/scope_policy.rs` 验证候选删除/弱化策略不改变必查义务，Markdown accepted 不构成批准。（Requirement: Protected actual change scope）
- [ ] 1.5 在 `src/git/diff.rs` 实现 `scan_scope` 对新增/删除/rename 两端/mode/symlink/submodule 的字节安全解析并决定路径编码/大小写 ADR；`tests/diff_scope.rs` 的越界 rename、非 UTF-8、子模块缺内容必须分别报违规或 missing coverage。（Requirement: Protected actual change scope）
- [ ] 1.6 在 `src/preflight.rs` 汇总只读 `CandidateRequest -> PreflightResult`，在 `tests/read_only_boundary.rs` 禁用 FlowGuard/写模块且使用无认证本地范围运行，确认返回 advisory 结果与冻结范围且用户 refs/index/worktree hash 不变。（Requirement: Read-only candidate dependency boundary）

## 2. Immutable candidate and parallel scope — GG-CANDIDATE（G0/G2 只读切片）

**Interfaces:** `prepare_candidate(FrozenSubject, CandidateRequest) -> CandidateSnapshot`；`register_binding(TaskScope, RepositoryRef) -> WorktreeBinding`；`analyze_impacts([CandidateSnapshot], GraphCoverage) -> ConflictObservation`。仅临时对象库可写；完整 envelope 留到组3。

- [ ] 2.1 在 `src/candidate.rs` 实现独立临时对象存储候选构造，冻结初始支持的 merge 方法/Git 功能探测 ADR；`tests/candidate_objects.rs` 覆盖 conflict、tree-preview、不同 parents 同 tree，断言只有真实候选 commit 能被准入绑定且原仓库不变。（Requirement: Exact queue candidate and parallel isolation）
- [ ] 2.2 在 `src/candidate.rs`、`adapters/queue_input.rs` 实现已解析队列事件输入端口与 candidate/base/group/member 固定，认证作为外部端口要求；`tests/queue_bindings.rs` 用 synthetic merge-group OID 重排组/推进 base，断言 PR-head 证据不满足新对象，未认证输入仅 advisory。（Requirement: Exact queue candidate and parallel isolation）
- [ ] 2.3 在 `src/worktree.rs` 实现 task/requirement/worktree 映射、唯一 ID、租约与目录复用规则；`tests/parallel_worktrees.rs` 用两个需求/两个 worktree 并发取消与复用，断言另一索引/输出不变且清理不删共享 ref。（Requirement: Exact queue candidate and parallel isolation）
- [ ] 2.4 在 `src/conflicts.rs` 冻结 provider/consumer/API/schema/事件的 `ConflictObservation` 和 required/advisory coverage，先提供 fixture 图谱端口；`tests/semantic_uncertainty.rs` 断言跨文件共享 API 报风险、缺索引保留 unknown、必需范围缺失不满足检查。（Requirement: Bounded Git observation and semantic uncertainty）
- [ ] 2.5 在 `schemas/candidate-snapshot.schema.json`、`fixtures/candidate/` 固定 repo/task/worktree/排序 requirementIds/candidate/base/group 与 source/baseline 摘要的序列化测试向量；`tests/candidate_schema.rs` 拒绝错误对象类型、重复/未排序 ID 和缺必须摘要，保留无基线 policy 的显式 nullable 规则。（Requirement: Immutable repository and subject binding）
- [ ] 2.6 在 `tests/gg_candidate_acceptance.rs` 和 `docs/gg-candidate-interface.md` 输出只读阶段契约与实际运行证据：无 FlowGuard/Engine 网络服务/写凭据的双需求、精确 queue candidate、drift/dirty/error fixtures 全通过，向 FG-GATE 提供 GG-CANDIDATE，不把组3–4完成设为先决条件。（Requirement: Read-only candidate dependency boundary）

## 3. Protocol and trusted evidence — G1/G2；分开协议与可信消费门槛

**Interfaces:** `project(PreflightResult, FrozenPolicy) -> GuardFacts`；`emit(BoundAttempt, EngineReport) -> GuardRunEnvelope`；`consume(EvidenceRefs, TrustPort, Binding) -> ValidatedEvidenceSet`；`append(Attempt) -> HistoryRef`。本域投影等 GE-CONTRACT/GE-ADAPTER；consume 等 GE-TRUST 与 policy 实际要求的 SG-BASELINE/AG-EVIDENCE/CG-ADAPTER/TG-EVIDENCE，不等待无关守卫全项目。

- [ ] 3.1 在 `src/evidence/projection.rs`、`schemas/git-domain-result.schema.json` 接入 GE-CONTRACT/ADAPTER 的固定版本 mapping；`tests/projection_vectors.rs` 校验 forbid_relation、enforce/review/advise、unknown field/version 拒绝和当前 engine 对象不能接纳 Git 字段。（Requirement: Strict protocol projection and version boundary）
- [ ] 3.2 在 `src/evidence/envelope.rs`、`src/cli.rs` 冻结 versioned check stdout/file/stderr、0/2/3/4 与取消契约（明确是否提供 `--report`）；`tests/cli_outcomes.rs` 覆盖 pre-binding 无 envelope、bound error/cancelled=null、partial=BLOCK/INDETERMINATE、envelope/report decision 一致和旧输出文件不能充当成功。（Requirement: Honest completion errors and decisions）
- [ ] 3.3 在 `src/evidence/consume.rs` 实现 GE-TRUST 的来源/引用 digest/同候选/coverage/批准基线核验端口并接入按需领域产物；`tests/evidence_trust.rs` 拒绝伪造 issuer、仅可重算报告、旧 baseline/PR-head，确认批准不改写 REQUIRE_APPROVAL、不修复 partial/error。（Requirement: Authenticated specialist evidence and baseline）
- [ ] 3.4 在 `src/evidence/store.rs` 选择事务唯一键/generation CAS 存储方案并建立 ADR，固定全量复用 key 与 append-only 历史；`tests/result_races.rs` 断言迟到旧 ALLOW 不能覆盖新 BLOCK、两需求不互相满足、取消重试有新 runId。（Requirement: Append-only freshness and late completion control）
- [ ] 3.5 在 `src/evidence/freshness.rs`、`src/evidence/audit.rs` 实现 binding/字节/依赖/规则/基线/analyzer/config/coverage 与 expiry/revocation 的资格失效，记录身份/原因/因果时间并配置访问/retention；`tests/freshness_audit.rs` 覆盖每类漂移及脱敏/受限删除，原报告不可篡改。（Requirement: Append-only freshness and late completion control；Requirement: Audited compatibility rollout and extensible ports）
- [ ] 3.6 在 `adapters/gitflow/compatibility.md`、`adapters/gitflow/` 调查并固定外部 provider 版本、命令/flags/report/退出语义，再实现 opt-in 映射；`tests/gitflow_differential.rs` 保存合法/违规/unknown/error 差分并逐项裁决，未核验能力不启用，原 CLI/CodeGuard native 数字退出码不改。（Requirement: Audited compatibility rollout and extensible ports）

## 4. Optional privileged executor — G2 写切片；默认关闭

**Interfaces:** `verify_grant(Grant, Binding, TrustPort) -> AuthorizedOperation`；`prepare_intent(AuthorizedOperation, RequestDigest) -> OperationState`；`apply(OperationState, PlatformPort) -> OperationReceipt`；`reconcile(OperationId, PlatformPort) -> OperationState`。开始前必须审查外部控制面授权契约与信任根；可能消费 FG-GATE，但绝不反向阻塞 GG-CANDIDATE。身份/审批/密钥 provider 在控制面，Engine 无签发责任。

- [ ] 4.1 在 `docs/execution-authorization-adr.md`、`src/execution/mod.rs` 定义 feature/配置默认禁用、独立身份凭据及外部授权审查门槛；`tests/executor_disabled.rs` 验证未启用/未审查时所有写端口拒绝但只读预检可用，ALLOW/FlowGuard gate/approved 布尔值都不能启用执行。（Requirement: Independently authorized optional execution）
- [ ] 4.2 在 `src/execution/grant.rs` 与 `schemas/operation-grant.schema.json` 依据已审查契约验证 issuer/actor/action/repo/candidate/target、not-before/expiry/revocation/scope/clock；`tests/grants.rs` 拒绝伪造 issuer、跨 repo、错误 action、过期撤回及不可信时间，worktree-create 不可升级为 merge。（Requirement: Independently authorized optional execution）
- [ ] 4.3 在 `src/execution/apply.rs`、`adapters/platform_write.rs` 实现独立 worktree/branch/push/merge 动作和原子 expected-target 前置条件；`tests/atomic_admission.rs` 并发推进临时 bare remote，断言未检查 OID 不写入、同 tree 不同 commit 不冒用、无 force/reset/delete 后备路径。（Requirement: Atomic admission of the verified candidate）
- [ ] 4.4 在 `src/execution/intent.rs`、`schemas/operation-receipt.schema.json` 实现写前 durable intent、operationId/request digest 事务唯一键与多执行器状态 CAS；`tests/intent_idempotency.rs` 覆盖同请求返回原状态、异请求拒绝、崩溃重启和双执行器争用至多一个有效写尝试。（Requirement: Durable idempotency and uncertain mutation reconciliation）
- [ ] 4.5 在 `src/execution/reconcile.rs` 实现 RECOVERY_REQUIRED 与平台 receipt/远端历史查询，只有确认未写且 grant 有效才可重试；`tests/write_recovery.rs` 注入写前/写后/回执前崩溃、响应丢失、目标再前进和取消，未知保持未知且不补偿回退。（Requirement: Durable idempotency and uncertain mutation reconciliation）

## 5. Platform integration, migration and release — G3/G4

**Interfaces:** 平台/MCP/CodeGraph 都消费已有 read-only 或显式 AuthorizedOperation 接口，不复制授权逻辑；生产独立分发等 GE-RELEASE，开发可用固定源码。跨仓库 change 地址见 [proposal](proposal.md#dependency-gates)。

- [ ] 5.1 在 `adapters/github/`、`tests/github_protected_queue.rs` 冻结首个平台事件认证/required check/精确 queue candidate 能力，隔离集成环境实测 Hook 绕过仍阻断、组重建重验和实际 receipt 匹配；无法证明原子精确准入时标不支持。（Requirement: Protected platform rollout and recovery proof）
- [ ] 5.2 在 `adapters/gitlab/`、`adapters/codegraph/` 与 `tests/extension_capabilities.rs` 单独验证第二平台语义及图谱版本/coverage，fixture 明确不支持字段/能力与缺图谱失败；不得套用 GitHub 字段或宣称 unknown 无冲突。（Requirement: Bounded Git observation and semantic uncertainty；Requirement: Audited compatibility rollout and extensible ports）
- [ ] 5.3 在 `adapters/mcp/`、`src/api.rs` 与 `tests/interface_parity.rs` 实现版本化只读端口及显式写能力协商，复用核心绑定/授权；对 CLI/API/MCP 同 fixture 比较范围、状态和诊断，未知能力/版本拒绝，默认不给写凭据。（Requirement: Audited compatibility rollout and extensible ports）
- [ ] 5.4 在 `docs/rollout-and-rollback.md`、`tests/rollout_rollback.rs` 落地 advisory→shadow→opt-in→enforcement 切换与独立 adapter/executor 回滚，保留历史/未知操作对账，差分未裁决不得强制；GE-RELEASE 后验证固定包版本与兼容矩阵，不改旧原生 CLI。（Requirement: Protected platform rollout and recovery proof；Requirement: Audited compatibility rollout and extensible ports）
- [ ] 5.5 在 `tests/end_to_end_guard_gates.rs` 与 `fixtures/end-to-end/` 联合已就绪门槛：两个并行需求、exact synthetic queue candidate、baseline/expiry/revocation、drift、late completion、CodeGuard native parity、取消与回滚；保存真实输入/digest/报告/exit/receipt 对应证据，确认 FG-GATE 不授予 grant 且 read-only 阶段无循环依赖。（Requirement: Read-only candidate dependency boundary；Requirement: Protected platform rollout and recovery proof）
