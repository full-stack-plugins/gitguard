# GitGuard 候选绑定治理增量提案

## Why

已检查 main `e03b5fd06d8b3d81d4bbbd11ff0fb70bea00d485` 及文档分支 `54bff26895dbd91875b8ff4af035fd4fcb6de509`。当前只有双语 README、架构/技术方案及共享集成草案，没有源码、CLI、测试、构建清单或旧 OpenSpec change。现有蓝图缺少可分步执行的规范与任务，容易把本地检查、流程门禁和写授权当作一个互相等待的大项目。本提案先交付独立只读 `GG-CANDIDATE`，再接证据，最后才考虑默认禁用的特权执行器。

依据：[架构](../../../docs/architecture.md)、[技术方案](../../../docs/technical-design.md)、[集成契约](../../../docs/integration-contract.md)、[跨仓库路线图](../../guard-roadmap.md)。本文及所有新规格都是未来要求，未完成的任务不是当前能力声明。

## What Changes

- 定义稳定仓库/任务/需求/Worktree 身份、真实 diff 范围和精确 commit/base/merge-group 候选绑定；候选预览仅在受控临时存储写对象。
- 建立并行任务隔离、受保护范围、未知冲突与能力不足的失败语义；只读核心不依赖 FlowGuard 或任何写服务。
- 在 GE-CONTRACT/GE-ADAPTER 后增加严格现行协议投影及独立 draft envelope；可信外部证据消费另等 GE-TRUST 和相关领域证据。
- 建立追加式运行审计、新鲜度及迟到完成控制；不把审批改写为 specialist ALLOW。
- 将窄范围 grant、原子目标条件、持久 intent 与未知写对账作为可选能力；审查外部授权契约之前不能启用。
- 将 GitFlow 固定版本调查、平台能力验证、语义图谱和 MCP/API 适配纳入分期，保留旧接口、可独立回滚。

## Capabilities

### New Capabilities

- `git-candidate-preflight`：只读仓库/范围观察、不可变候选、并行任务绑定、安全进程边界。
- `git-evidence-integration`：现行事实投影、版本化结果、证据消费、失效/审计与兼容迁移。
- `git-controlled-execution`：默认关闭、独立授权的写入、原子执行、幂等对账和渐进平台准入。

### Modified Capabilities

无。仓库没有既有 OpenSpec capability；本 change 不替换外部插件或其他守卫所有权。

## Impact

未来新增 `Cargo.toml`、`src/{git,subject,scope,candidate,worktree,evidence,cli,execution}/`、`schemas/`、`adapters/`、`tests/`、`fixtures/`。这些路径现在不存在，依赖未安装。Rust 2024/Serde/Clap 和 Git CLI 是候选方案，Git 2.41+ 仅为初始测试目标，最低支持能力待测试后冻结。当前 `guard.partme.ai/v1alpha1` 严格格式与 `forbid_relation` 保持不变；`guard.integration/v1alpha1` 仍为独立草案，不假定 N/N-1 兼容。

## Dependency Gates

| 阶段 | 前置门槛 | 对外产物 |
|---|---|---|
| 任务组 1–2 | 本地 Git fixtures、受信配置接口；可与 GE 平行 | GG-CANDIDATE：类型化、只读、可验证的精确候选与覆盖 |
| 任务组 3 的本域投影 | GE-CONTRACT + GE-ADAPTER | GitGuard 自己的合法 facts/report/envelope |
| 任务组 3 的可信消费 | GE-TRUST + 实际要求的 SG-BASELINE / AG-EVIDENCE / CG-ADAPTER / TG-EVIDENCE | 已认证且绑定匹配的外部证据集合 |
| FlowGuard FG-GATE | 消费 GG-CANDIDATE 与相关可信证据 | FlowGuard 自己限定范围的技术 gate 报告；不是 Git 授权 |
| 任务组 4 可选执行 | 已完成只读阶段、所需 gate 输出、外部控制面授权契约审查、平台原子能力验证 | 窄范围授权写入与 receipt；不反向阻塞 GG-CANDIDATE |
| 任务组 5 生产发布 | 所需以上门槛；独立分发等 GE-RELEASE | 可回滚集成与跨仓库验收记录 |

跨仓库依赖：[GuardEngine](https://github.com/full-stack-plugins/guardengine/tree/docs/guard-design-20261009/openspec/changes/add-versioned-guard-integration-contracts)、[SpecGuard](https://github.com/full-stack-plugins/specguard/tree/docs/guard-design-20261009/openspec/changes/add-specification-baseline-analysis)、[ArchGuard](https://github.com/full-stack-plugins/archguard/tree/docs/guard-design-20261009/openspec/changes/extend-architecture-analysis-and-evidence)、[CodeGuard](https://github.com/full-stack-plugins/codeguard/tree/docs/guard-design-20261009/openspec/changes/add-guardengine-compatibility-adapter)、[TestGuard](https://github.com/full-stack-plugins/testguard/tree/docs/guard-design-20261009/openspec/changes/add-test-obligation-evidence-pipeline)、[FlowGuard](https://github.com/full-stack-plugins/flowguard/tree/docs/guard-design-20261009/openspec/changes/add-evidence-bound-workflow-gates)。这些是阶段接口依赖，不是等待整个仓库完成。

## Non-goals and Open Decisions

不实现需求批准、架构/代码/测试分析本身，不让 GuardEngine 签发许可或操作 Git refs，不给本地 Hook 强制能力。独立 gitflow-plugin 未检查，配置/语言/退出码不得预设。签名/撤回 provider、持久存储、候选构造策略、字节路径表示、平台最低能力和 retention 是执行前决策；默认无写权限、不支持的能力明确失败、不进行隐式迁移。批准不能覆盖 partial、错误、取消或篡改 specialist 的 REQUIRE_APPROVAL。
