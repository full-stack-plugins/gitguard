## ADDED Requirements

### Requirement: Strict protocol projection and version boundary

GitGuard SHALL 在 GE-CONTRACT/GE-ADAPTER 后将本域关系映射为现行 `guard.partme.ai/v1alpha1` 支持的 GuardFacts 与精确 `forbid_relation`，独立输出协商后的 `guard.integration/v1alpha1` envelope；MUST 拒绝未知字段/不支持版本，不隐含 N/N-1 支持。Git 对象/授权字段不得加入现行引擎对象。

#### Scenario: Domain metadata targets an engine report

- **WHEN** 输入试图把 candidateOid 或 grant 插入现行 GuardReport
- **THEN** 验证拒绝该对象，领域字段只能通过独立 domain artifact/envelope 传递

### Requirement: Honest completion errors and decisions

GitGuard MUST 冻结 binding、producer 与 required coverage 后才发 envelope。完成且 engine-backed 的 decision MUST 等于引用 report；合法 partial facts SHALL 产生 BLOCK/INDETERMINATE；工具错误/取消 SHALL 为独立运行状态且 decision 为 null。目标 check 退出码 SHALL 为 0/2/3/4，stderr 为诊断，stdout/file 行为须在版本化 CLI 契约中冻结。

#### Scenario: Bound analyzer failure is not a policy pass

- **WHEN** 完整绑定后分析器超时或取消
- **THEN** 返回 error/cancelled、null decision 和失败退出状态，不复用旧 ALLOW 作为本次结果

#### Scenario: Partial valid facts and a review report

- **WHEN** 一次运行得到合法 partial facts，另一次完整运行得到 REQUIRE_APPROVAL
- **THEN** 前者为 BLOCK/INDETERMINATE，后者 envelope 保留 REQUIRE_APPROVAL；批准记录不改写二者 specialist 判定

### Requirement: Authenticated specialist evidence and baseline

GitGuard MUST 在 GE-TRUST 与所需领域产物可用后才可信消费外部证据，验证来源、摘要、相同候选/base/group、范围及不可变批准基线。verify 重算与文件 digest 不得充当身份认证；ALLOW 不授予 Git 权限。

#### Scenario: Old baseline and untrusted report

- **WHEN** 报告摘要可重算但来源未认证，或基线 ref/digest 与批准 scope 不同
- **THEN** 必需义务仍未满足，不能从 Markdown accepted 或布尔批准字段恢复资格

### Requirement: Append-only freshness and late completion control

GitGuard SHALL 追加记录运行，以完整候选/基线/规则/分析器/config/coverage key 控制复用；MUST 以 generation CAS 更新当前结果。候选/base/group、字节、依赖、基线、policy、分析器/coverage 变化或批准到期/撤回 SHALL 触发受影响资格失效，保留原始结果。

#### Scenario: Late old allow follows newer block

- **WHEN** 旧候选 ALLOW 晚于新候选 BLOCK 完成
- **THEN** 旧结果仅追加到自身历史，不能替换当前候选的 BLOCK

#### Scenario: Cached result loses approval eligibility

- **WHEN** 技术结果缓存命中但所需批准已过期或撤回
- **THEN** 控制面重评资格并拒绝受影响门禁，不删除或改写原始技术报告

### Requirement: Audited compatibility rollout and extensible ports

GitGuard SHALL 记录认证身份、绑定、摘要、因果关系、结果及受信时间，配置保留/访问/删除并脱敏。扩展 adapter MUST 声明能力与失败语义；旧 GitFlow 行为须固定版本调查及差分后才迁移，不修改旧 CLI/CodeGuard 原生退出码。rollout SHALL 可分离回滚本域 adapter 与可选执行器，保留历史。

#### Scenario: Legacy behavior differs during shadow comparison

- **WHEN** 固定 provider 的 native 与新 adapter 对同一 fixture 产生差异
- **THEN** 记录差异和命令/flags/report 语义，阻止未裁决能力进入 enforcement，旧接口仍可使用

#### Scenario: Unsupported extension or rollback

- **WHEN** MCP/平台 adapter 缺必需能力，或操作员关闭新 adapter
- **THEN** 不把不支持解释成通过，回退到已验证路径或明确阻断，审计历史保持可读且无凭据
