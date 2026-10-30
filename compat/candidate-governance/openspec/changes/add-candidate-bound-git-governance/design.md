# GitGuard 增量设计

## Context

基线 main `e03b5fd06d8b3d81d4bbbd11ff0fb70bea00d485` 只有四份设计文档；规划前 HEAD `54bff26895dbd91875b8ff4af035fd4fcb6de509` 增加/修订共享契约但仍无运行实现、测试或旧 change。本 change 不重复未存在的实现；它将 G0–G4 蓝图转为独立验收的阶段。

依据：[架构](../../../docs/architecture.md)、[技术方案](../../../docs/technical-design.md)、[集成契约](../../../docs/integration-contract.md)、[路线图](../../guard-roadmap.md)。跨仓库 change 链接与门槛见 [proposal](proposal.md#dependency-gates)。

## Goals / Non-Goals

目标为只读 GG-CANDIDATE、可验证的本域证据、可选安全写入。六个守卫的领域所有权不变，GuardEngine 仅提供通用 Contract/Rule/Evidence；FlowGuard 聚合流程义务，不发 Git 操作许可。CLI 不自行接受 `approved=true` 或本地公钥作为可信授权。

## Decisions

### 1. 先冻结只读接口，避免循环依赖

组 1–2 以独立输入 `TaskScope + RepositoryRef + CandidateRequest` 输出 `CandidateSnapshot + ChangeSet + Coverage`，不调用 FlowGuard。任务范围可以由本地受信配置/控制面 stub 提供；未认证配置只能产生 advisory 预览，不能冒称批准范围。SG 基线绑定与其他守卫集成在后续门槛加入。GG-CANDIDATE 是本地类型化能力，发布跨守卫 envelope 另等 GE-CONTRACT/ADAPTER。FlowGuard 消费此接口，无需等待 GitGuard 写执行器。

阶段 DAG：`组1 → 组2/GG-CANDIDATE → 组3可信集成 → FG-GATE（外部） → 可选组4`；组3依赖 GE-CONTRACT/ADAPTER/按需 GE-TRUST 和领域证据，组4额外依赖外部授权审查。组5独立分发受 GE-RELEASE 约束。FlowGuard 输出仅为输入证据，绝不替代 actor/action grant。

### 2. 模块与接口边界（未来路径）

| 模块 | 输入 → 输出 | 副作用 |
|---|---|---|
| `src/git/{runner,repository,objects}.rs` | argv/显式根 → 受限字节输出/RepositoryRef | 无远端写、无隐藏 fetch；限时与输出限制 |
| `src/subject.rs`, `src/scope.rs` | 受保护 TaskScope + 固定对象 → Binding/ChangeSet | 观察真实 diff，区分 index/worktree/commit |
| `src/worktree.rs` | repo/task/requirement 映射 → 唯一 worktreeId/租约 | 只读注册；真实创建由可选执行端负责 |
| `src/candidate.rs` | 固定 base/head 或实际 queue candidate → CandidateSnapshot | 仅临时对象库构造，不改用户索引/refs |
| `src/conflicts.rs` | 多任务 snapshot + 图谱版本 → 风险/unknown/coverage | 不执行候选代码；不替代测试 |
| `src/evidence/{projection,envelope,consume,store}.rs` | 冻结绑定/观察/引用 → facts、envelope、历史 | 输出/存储受控，不持有写 Git 凭据 |
| `src/cli.rs`, `adapters/{github,gitlab,mcp}/` | 版本化请求 → 同一核心结果 | 本地端口默认只读，平台能力显式协商 |
| `src/execution/{grant,intent,apply,reconcile}.rs` | 已认证许可+候选+预期 ref → OperationReceipt | 默认禁用；经审查后窄范围原子写 |

这些是待创建模块，无现有函数/依赖可复用声明。构建清单随第一个可执行只读切片引入，serde/clap/异步库具体版本由兼容测试决策。

### 3. 身份、范围与精确对象

稳定 repoId 来自注册接口，URL 不是身份；解析 common Git dir 和显式 root，拒绝越界和不可信 alternate 对象来源。OID 使用仓库算法验证，支持能力由 SHA-1/SHA-256 fixtures 证明。绑定含 sorted/unique requirementIds、taskId、worktreeId、candidateOid、baseOid、mergeGroupId、sourceSnapshotDigest、baselineDigest；policy revision、digest、schema version、crate semver 各自独立。基线无要求时 nullable 的规则必须来自冻结 policy。

全量扫描 rename 两端、删除、mode、symlink、子模块指针；子模块内容未扫描即不算该范围完整。dirty 内容使用自己的 source snapshot 与 subject，不能声称权威 merge evidence。candidate 必须是提交，tree preview 不能填 candidateOid。两个父关系不同但 tree 相同的提交不能默认复用；实际 queue candidate 优先使用平台对象。

### 4. 结果、状态与错误

运行 `queued → running → completed/error/cancelled`；eligibility/stale 为控制面派生状态，不改 archived envelope。领域状态可有 CONFLICT/BLOCKED/STALE；可选写操作独立维护 PREPARED/EXECUTING/CONFIRMED/RECOVERY_REQUIRED，不能将 CLI 退出 0 当作 MERGED。

绑定/producer/coverage 冻结前错误仅独立 transport diagnostic，不能填空 OID 或生成 envelope。冻结后 tool error/cancelled 的 decision 为 null；合法 partial facts 的完成报告为 BLOCK/INDETERMINATE。engine-backed envelope decision 必须等于 report.decision。未签名 report verify 仅重算，生产可信消费需 GE-TRUST 接口认证来源和摘要。审批可以满足 FlowGuard 自己的评审义务，不能改写被引用的 REQUIRE_APPROVAL，不可修复 partial/error。

未来 check 的 JSON/stdout、stderr 诊断、0/2/3/4 与 schema 作为明确版本契约；`--report` 尚无实现，采用前需冻结写文件/输出行为。写命令独立 receipt 契约。旧插件原始退出码不改，适配层须按命令/flags/report 映射。

### 5. 并发、失效与审计

每个任务/需求保留独立义务、历史、审批；同字节提取仅在 config/coverage 等完整 key 一致时共享。key 包含候选/base/group、snapshot、基线/规则 digest、producer/analyzer/config 和 required coverage。写历史 append-only，更新 current pointer 使用 generation CAS。旧 PASS 可存档，不能覆盖新 FAIL。

队列重排、目标推进、字节/依赖/基线/规则/分析器/覆盖改变使受影响义务失效；批准到期/撤回重评资格而非篡改原结果。审计记录认证身份、引用摘要、时间、原因和因果前驱；配置访问/保留/删除策略，脱敏令牌和最小源片段。锁只保护共享动作，不授予权限。

### 6. 可选执行与恢复

默认禁用全部 Git 写端口。外部授权契约须审查 issuer/actor/action/repo/candidate/expected target、审批 scope、not-before/expiry、撤回和时钟。grant 与 FlowGuard 技术 gate 是独立输入。controller 负责发放身份、批准和短时凭据，Engine/GitGuard 分析器不保管签发密钥。

写前事务存 intent；operationId 同摘要返回已有 receipt，不同摘要拒绝。多执行器争用唯一键/CAS 后才执行；目标 ref 检查必须与更新原子化。拒绝普通“先读后 push”替代 CAS。响应丢失后 RECOVERY_REQUIRED，先读取平台操作回执与远端历史，不能只比较当前 ref（可能再次前进）；不确定就保留未知。确认未发生、授权仍有效才能重试；禁止自动 force/reset/delete/补偿回退。

### 7. 迁移与扩展

先固定外部 gitflow-plugin 版本并调查原生接口，再做正常/违规/unknown/错误的差分，所有差异显式裁决；目前未验证其实现。GitHub/GitLab 平台能力各自验证，不能保证精确对象准入时明确不支持；CodeGraph unknown 保留，MCP/API 复用同核心和授权。初始本地 advisory → shadow → opt-in required checks → explicit enforcement，可分别关闭 adapter/写端口并恢复已验证旧路径，保留不可变历史，不改 CodeGuard 原生接口。

## Risks / Trade-offs

对象构造会写临时存储，故“只读”精确定义为用户仓库/远端无修改，不是进程零磁盘写。路径字节/大小写、子模块递归、Git 最低版本、merge/squash/rebase 支持、事务存储和租约、授权 provider 和 retention 需在所属任务中形成 ADR 与失败 fixtures；可逆默认是保守不支持、单 provider 接口、最小保留，不虚构已选服务。

## Validation Strategy

真实临时 Git/bare remote 验证副作用、对象与漂移；并行两需求/多 worktree 验证隔离、迟到结果；黄金向量验证严格字段/version/partial/error；伪造/过期/撤回许可验证拒绝；平台测试验证 Hook 绕过仍不能合入。跨仓库 END-TO-END 在相关门槛后验证 exact synthetic queue candidate、两需求、expiry/revocation、drift、late completion、CodeGuard native parity 和回滚。当前只编写计划，不运行这些未来测试，不以文档检查代替运行证据。
