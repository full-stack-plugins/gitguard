## ADDED Requirements

### Requirement: Read-only candidate dependency boundary

GitGuard SHALL 独立提供类型化 GG-CANDIDATE 只读预检，不依赖 FlowGuard 完成、其他领域检查完成或任何特权写执行器。未认证本地范围 MUST 仅作为 advisory 输入，不得宣称批准范围；跨守卫 envelope 发布 MUST 等待 GE-CONTRACT/GE-ADAPTER。

#### Scenario: Candidate preflight without workflow service

- **WHEN** 本地固定 Git 对象与显式任务范围有效且 FlowGuard/写执行器均未部署
- **THEN** 系统返回带 scope/coverage 的类型化候选预检，声明授权未评估，不执行远端写

### Requirement: Immutable repository and subject binding

GitGuard MUST 验证可信 repoId、实际仓库根、对象算法和真实 commit 对象，并绑定 taskId/worktreeId、排序去重 requirementIds、candidate/base/group 与快照摘要；worktree/index/commit/tree-preview 不得互相冒充。绑定前失败 MUST 返回独立诊断，不伪造 envelope 必填字段。

#### Scenario: Dirty checkout is not authoritative merge evidence

- **WHEN** 工作目录字节与 HEAD 不同
- **THEN** 观察结果标明 dirty subject 与 sourceSnapshotDigest，不得以 HEAD 报告代替权威候选证据

#### Scenario: Unknown repository cannot create an envelope

- **WHEN** repoId 无法验证或候选对象不存在
- **THEN** 系统失败并输出独立诊断，不生成空 OID 的 GuardRunEnvelope

### Requirement: Protected actual change scope

GitGuard SHALL 从固定对象真实 diff 检查新增/删除/修改、rename 两端、mode、symlink 和 submodule，冻结受保护范围与基线引用，不信任候选自改策略。所需范围不可观察 MUST 保留不完整，不得判定完整安全。

#### Scenario: Rename attempts to escape the approved scope

- **WHEN** rename 的来源或目标路径违反冻结范围，或候选试图删除必查策略
- **THEN** 系统记录范围违规，不能用新的候选策略消除该义务

#### Scenario: Submodule pointer does not prove contents

- **WHEN** 仅可观察子模块 OID 而 policy 要求递归内容分析
- **THEN** coverage 标明缺失内容，不能宣称完整

### Requirement: Exact queue candidate and parallel isolation

GitGuard MUST 固定实际 queue candidate/base/group 与组成员快照；构造预览仅写受控临时存储，不改变用户 refs/index。每个 task/requirement/worktree MUST 保有独立绑定和义务，目录复用须更换 worktreeId。

#### Scenario: Queue rebuild invalidates a branch check

- **WHEN** PR HEAD 检查完成后队列产生新候选或重排成员
- **THEN** 旧 branch/queue 绑定不得满足新候选预检，必须重新绑定实际提交而非仅比较 tree

#### Scenario: Two requirements share one repository

- **WHEN** 两个需求在独立 worktree 并发运行且其中一个取消或复用目录
- **THEN** 另一任务的索引、绑定和输出不变，取消任务不能删除共享 refs 或他人资源

### Requirement: Bounded Git observation and semantic uncertainty

GitGuard MUST 用受限 argv 子进程、NUL 安全解析、路径边界和资源限制观察 Git，禁止隐藏 fetch、策略脚本、外部 diff/textconv/hook 与写凭据。语义图谱 SHALL 声明版本和覆盖，unknown 不得转为无冲突；必需图谱缺失不能放行。

#### Scenario: Malicious repository configuration

- **WHEN** 不可信仓库配置试图通过 helper/diff/环境或越界路径执行外部命令
- **THEN** 观察器拒绝或隔离该入口，用户仓库/远端状态保持不变，诊断不泄漏凭据

#### Scenario: Disjoint files share an incomplete API graph

- **WHEN** 两任务改不同文件但共同依赖 API，索引覆盖不完整
- **THEN** 系统报告风险或 unknown 及缺失范围，不声称无冲突，也不替代实际编译测试
