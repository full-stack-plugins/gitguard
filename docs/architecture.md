# GitGuard — Git 与变更治理架构

> GitGuard | 目标架构 V0.1 | 当前未实现产品引擎，仅有初始仓库；GitFlow 现有能力是待核验的独立依赖 | 2026-10-09

## 1. 定位与权责

GitGuard 针对**AI 多任务并行开发和合并风险**，确保“该任务被允许修改什么、修改基于哪个版本、是否影响其他任务、此次合入的是哪一个最终候选”。它不决定需求正确性（SpecGuard）、架构好坏（ArchGuard）、代码质量（CodeGuard）或测试充分性（TestGuard）。FlowGuard 负责批准/阶段；GuardEngine 提供通用策略、规则及证据协议，真正执行 Git 写操作及校验目标版本属于 GitGuard 与可信 Git 平台。

局部 Git 检查 PASS **不等于**获得受保护分支写权限。Worktree 是目录隔离，不是 OS 级安全沙箱；受信合入必须由独立权限/CI/分支保护机制实施。

## 2. 领域组件与数据流

~~~text
FlowGuard approved task + Spec/Arch contracts
                       │
                       ▼
        Change Scope / Task Binding Registry
                       │
                       ▼
             Git Repository Observer
    HEAD / index / worktree / diff / refs / target
                       │
           ┌───────────┼──────────┐
           ▼           ▼          ▼
     Branch Policy   Scope      Conflict
     & Baseline      Rules      Analyzer
                       │
                       ▼
              Candidate Builder
       final merge tree (base + task changes)
                       │
                       ▼
           GitGuard Facts + Evidence
                       │
                   GuardEngine
                       │
                 FlowGuard Gate
                       │
            Trusted Git Executor
      atomic expected-ref check / protected merge
~~~

### 2.1 Branch Policy & Worktree

显式声明任务分支来源、命名、所有权、状态机和 Worktree 关联。检查 commit/push 的动作对象必须基于真实仓库根，不得误取命令调用者的 cwd；禁止自动 force-push、reset、删除分支或悄悄修改 Git 用户配置。Worktree 仅用于并行目录分离，不能代替权限隔离或共享凭据控制。

### 2.2 Change Scope

任务契约定义 allowedPaths/forbiddenPaths/allowedModules/approvedContracts，以及创建、删除、重命名、文件模式和子模块变更的允许范围。比较的是**实际 diff**，不能相信 Agent 自报“只改了业务目录”；变更强制规则、GitHub Actions、验收文件时必须执行保护策略或独立评审。

### 2.3 Semantic Conflict (advisory)

关联多个任务的 changedFiles、changedSymbols、consumedPublicApis、read/write sets、schema/migration、公开事件/配置契约。即使文件不重合，共享 API 提供者和消费者也可能不兼容。代码图谱不完整时标注 UNKNOWN，不宣布“无冲突”；方法归属/领域设计最终由 ArchGuard 负责，真正的合并仍须编译和 TestGuard 回归。

### 2.4 Final Merge Candidate

最终合并候选必须记录 baseCommit、targetRef+targetOid、headCommit、mergeTreeOid、contractSnapshotDigest、verificationRun、operationId。目标 main 改变即使此前本地验证绿也必须失效；针对最新目标重新构造合入候选执行检查。执行合并必须采用目标 ref 期望值 CAS/merge queue 等平台机制，防止 TOCTOU。不能用 PR HEAD 的 PASS 代表最终合并候选 PASS。

## 3. 状态与策略

建议状态：REQUESTED → BASELINE_LOCKED → WORKTREE_ALLOCATED → CHANGES_OBSERVED → CANDIDATE_READY → VERIFIED → MERGE_ELIGIBLE → MERGED；另有 BLOCKED、CONFLICT、STALE、UNKNOWN 和 RECOVERY_REQUIRED。GitGuard 专注 Git 生命周期及操作约束，FlowGuard 管理整体研发阶段；任何 Git 状态推进都不能自动审批产品/技术要求。

统一执行状态、规则结果、覆盖与权限：ENFORCE 针对禁止文件/分支/强制目标版本；REVIEW 针对冲突风险；ADVISE 提供习惯建议。错误的 Git CLI 执行或未知推送结果必须进入对账，不可盲目重放。

## 4. 既有生态关系

现有 [gitflow-plugin](https://github.com/full-stack-plugins/gitflow-plugin) 已有 Git 规则、Python 标准库实现、独立 `.gitflow/` 配置及显式 apply。GitGuard 不重新实现后直接宣称替代它。首期将其作为固定版本的 legacy provider/adapter，并以合法/违规/unknown/恢复测试做差分验证；迁移完成后才切换唯一规则所有权。既有 CodeGuard/codeguard-plugin 的 commit/push 反馈只作为局部证据，不能自签最终 MERGE_ELIGIBLE。

## 5. Trust Boundary & ADR

- GG-ADR-001：被检查的对象区分 worktree、index、commit 和 final merge candidate，报告不能互相冒用。
- GG-ADR-002：必须检查目标 ref/OID 漂移，合并操作使用受信身份和原子期望版本条件。
- GG-ADR-003：冲突图谱先风险预警，未知不得转换成“安全”。
- GG-ADR-004：复用现有 gitflow-plugin 的规则并保持迁移单一所有权。
- GG-ADR-005：CLI Hook 仅协作反馈；受保护分支、required checks 和可信 CI 才提供强制边界。
- GG-ADR-006：未知 Git 写结果先对账而非立即重试；不自动强推或删历史。

## 6. 正反例验收

| 情形 | 预期 |
|---|---|
| 允许路径变更、正确来源分支、最终候选验证完整 | 本域条件满足 |
| Agent 跨目录、删除门禁脚本或改权限策略 | 根据可信规则 BLOCK |
| 目标 main 在检查后前进 | 旧报告 STALE、重新合并验证 |
| 两任务改不同文件但共享 API 不兼容 | REVIEW/冲突预警 + 真实编译测试 |
| 合并后 Git 操作结果不确定 | 查询远端 refs 对账，不重复强推 |
| 旧 Commit 的证据用于新 merge tree | 证据不匹配，BLOCK |
| 只存在本地 Hook 但服务器没开保护 | 不能宣称 enforced |

实施所需 API、状态、命令与失败恢复见 [技术方案](technical-design.md)。
