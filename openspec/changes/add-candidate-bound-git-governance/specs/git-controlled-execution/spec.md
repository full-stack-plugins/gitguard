## ADDED Requirements

### Requirement: Independently authorized optional execution

GitGuard MUST 默认关闭 Git 写端口；只有外部控制面授权契约和信任根审查完成后，才校验 issuer/actor/action/repo/candidate/expected target、审批 scope、有效期与撤回并使用短时凭据。FlowGuard gate、技术 ALLOW 或自报 approved 不得代替 grant；GG-CANDIDATE 不依赖该能力。

#### Scenario: Passing gate without valid grant

- **WHEN** FlowGuard gate 满足但 grant 缺失、到期、撤回或绑定错误
- **THEN** 写操作被拒绝，只读候选查询仍可独立工作

### Requirement: Atomic admission of the verified candidate

GitGuard SHALL 仅在平台支持原子 expected-target 条件和精确候选准入时执行受保护写入；target 漂移或平台生成不同对象 MUST 使旧资格失效。工作树/分支创建、push、merge 各动作 MUST 使用独立授权，不得自动 force/reset/delete。

#### Scenario: Target moves between verification and apply

- **WHEN** 检查后目标 ref 推进，或实际落地提交不再等于已检查候选
- **THEN** 原子条件拒绝旧写入并触发新候选验证，不能普通重试 push 绕过

### Requirement: Durable idempotency and uncertain mutation reconciliation

GitGuard MUST 写前持久化 operationId 与请求摘要 intent，以事务唯一键/CAS 保证多执行器争用安全。同 ID 同请求返回已有状态，异请求拒绝。未知写结果 MUST 先查平台 receipt 与远端历史，未确认前不得重放或补偿回滚。

#### Scenario: Response loss after accepted write

- **WHEN** 远端写成功后连接中断且目标又继续前进
- **THEN** 操作进入 RECOVERY_REQUIRED，按 receipt/历史对账而非仅当前 ref 相等判断；仍不确定就保持未知

#### Scenario: Duplicate operation changes the payload

- **WHEN** 第二执行器使用相同 operationId 但不同候选或目标摘要
- **THEN** 系统拒绝第二请求，不产生第二次写

### Requirement: Protected platform rollout and recovery proof

GitGuard SHALL 逐平台验证独立 required checks、事件身份、精确队列对象和绕过保护，在 advisory/shadow/opt-in 后才显式 enforcement。写入故障、取消、回滚及多需求联合场景 MUST 有真实隔离环境证据；无法保证对象一致的平台不得声明强制支持。

#### Scenario: Local hook is bypassed

- **WHEN** 集成测试绕过本地 Hook 尝试合入未满足必需候选检查的提交
- **THEN** 受保护平台阻止准入，测试记录实际候选/检查/回执对应关系

#### Scenario: Execution feature is rolled back

- **WHEN** 特权执行功能被关闭或取消与远端写并发
- **THEN** 停止新写调度、保留未知操作对账和审计能力，不假设取消撤销已发生写入
