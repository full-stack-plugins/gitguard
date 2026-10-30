# GitGuard — Git 与变更治理架构

> 本文描述完整目标架构；历史main e03b5fd在2026-10-09确为纯文档。当前实现分支已有受限本地Rust库、check CLI、真实Git/GE产物及history/CAS，13/28任务已按本地profile独立验收；准确事实见[实施进度](implementation-progress.md)。托管强制门禁、生产provider和可用写执行器尚未实现，目标架构不等于当前生产能力。

## 1. 定位、输入输出与权责

GitGuard 回答“任务允许修改什么、基于哪个不可变版本、与哪些并行任务相互影响、最终检查和合入是否为同一对象”。它面向本地任务隔离、CI 变更范围校验、合并队列和受控 Git 写入。离线只读检查只能证明本地快照，不能证明远端当前状态。

| 边界 | 所有者与责任 |
|---|---|
| 需求及批准基线含义 | SpecGuard；可信控制面认证批准者、范围、时效 |
| 架构、代码、测试 | 分别由 ArchGuard、CodeGuard、TestGuard 产生自己的领域证据 |
| 流程推进及所需审批 | FlowGuard；不把技术 ALLOW 当作授权 |
| 仓库、分支、范围、候选、写安全 | GitGuard 与受保护 Git 平台 |
| 通用契约、规则求值、确定性证据 | GuardEngine；不解析 Git 领域策略、不发放操作许可、不执行合并 |

输入：可信需求基线和范围契约、任务/Worktree 身份、解析后的 Git 对象、明确来源和观察时间的目标快照、其他守卫的候选绑定证据。输出：实际 ChangeSet、范围/分支检查、带覆盖说明的冲突观察、MergeCandidate、协议事实/报告引用、可审计的操作回执。跨守卫载体见[集成契约](integration-contract.md)，领域详情不任意塞入现有协议。

## 2. 组件与数据流（目标）

~~~text
受保护基线/范围 + FlowGuard task binding
                    ↓
       Task / Requirement Binding Registry
                    ↓
       Repository Observer（真实对象、路径、refs）
                    ↓
    Branch Policy / Scope Scanner / Conflict Analyzer
                    ↓
       Candidate Builder（受控临时对象库/工作区）
                    ↓
        Immutable Candidate + Domain Observations
                    ↓
    Facts Adapter → GuardEngine → Evidence Registry
                    ↓
      FlowGuard Gate + Authenticated Grant Verifier
                    ↓
    Trusted Git Executor → Protected Host / Atomic CAS
                    ↓
        Durable Receipt / Read-only Reconciliation
~~~

只读观察与候选构造分开：构造提交/树可能写临时对象，不能对用户索引、工作目录或 refs 产生隐藏写入。写执行器与分析器分进程/身份，分析器默认无远端写权限。运行候选测试属于独立沙箱，不能继承执行器密钥。

### 2.1 仓库、分支与 Worktree

仓库身份来自可信注册映射，URL 字符串不足以唯一认证仓库。区分 common Git dir、实际 worktree 路径、worktreeId、taskId 和任务分支；通过 Git 解析仓库边界，不根据调用者 cwd 猜测对象。支持 SHA-1/SHA-256 身份标记，不假定所有 OID 长度固定。Worktree 共享对象库、refs、部分配置和凭据，并不提供安全隔离。

每个任务拥有独立索引/工作目录/输出命名空间；一个需求可以有多个任务，一个候选可以关联多个需求，必须显式记录映射。复用目录不复用 worktreeId；任务取消、worktree 删除不得删共享 refs 或另一任务对象。生命周期更新需租约/版本条件；共享 ref 写入仍由原子前置条件保护，锁本身不是授权。

### 2.2 范围与并行语义冲突

范围契约定义允许/禁止路径、模块及受保护公共契约；扫描实际 diff 的新增、修改、删除、重命名两端、文件模式、符号链接和子模块 OID。默认重命名不是逃逸手段，子模块指针检查不等于分析其内容，缺少所需对象则不能宣称完整。策略文件、CI、验收基线和授权配置需要受保护规则及独立审批。

跨任务观察共享符号、API provider/consumer、读写集合、Schema/migration、事件与配置契约。不同文件也可构成风险；同一文件也不必然语义冲突。索引版本、分析范围和 unknown 必须保留。语义图谱默认用于建议/评审，不能代替 ArchGuard、编译或 TestGuard；被声明为必需的分析缺失时阻断相应门禁。

### 2.3 不可变候选与并发证据

候选核心绑定为 `repoId/taskId/worktreeId/requirementIds/candidateOid/baseOid/mergeGroupId`，语义以共享草案为准；还需领域侧记录 source head、target ref/expected OID、merge tree、Git 算法、构造方法和范围/基线摘要。`candidateOid` 必须指向实际检查的 Git 候选提交；tree 相等不代表提交及父关系相等。若尚只有 tree preview，则不得登记为可合入候选。

需求基线绑定不可变 digest/ref 与外部认证批准记录，不能以 `accepted: true` 或 Markdown 文字充当审批。结果键覆盖候选绑定、基线/规则版本、分析器/覆盖和输入摘要；运行有独立 runId。结果采用追加存储；“最新结果”索引只能以版本条件更新。旧任务、旧 base 或晚到的取消运行均不能覆盖新候选结果。并行任务合并形成的新候选必须重新检查，不能拼接各分支的 ALLOW。

## 3. 合并队列与强制边界

1. 从可信平台事件取得队列组、实际 base 和 candidate；事件先认证，再读取/核实对象，不能直接信任 PR 提交的 JSON。
2. 绑定组成员及候选，执行所有必需检查，并核对报告输入摘要、分析器及覆盖。仅 PR HEAD 的绿灯不满足队列候选检查。
3. 目标推进、组成员重排、候选重建、基线/规则/分析器/覆盖变化均使相关结果失效；审批撤回/到期至少撤销授权可用性，门禁重评，不能沿用过期许可。
4. FlowGuard 满足生命周期门禁后，由可信控制面签发窄范围 grant。执行器再次核对 grant、候选、目标 OID、审批、证据与当前保护策略。
5. 平台以精确候选准入/merge queue 或等价原子 expected-ref 条件完成写入；“查询目标后再普通写入”不能防 TOCTOU。若平台重建不同提交，必须重验或使用能保证相同候选的机制。
6. 持久化执行回执。超时或断连先对账，不把错误解释成未发生写入。

本地 Hook 只提供反馈；保护分支、独立 required checks、受限身份和实际绕过测试才证明强制实施。GitGuard 的 ALLOW 不是合并或发布授权。

## 4. 状态机与失效（目标）

| 状态迁移 | 必须满足的条件 |
|---|---|
| REQUESTED → BASELINE_LOCKED | 范围/基线可解析且批准身份、范围有效 |
| BASELINE_LOCKED → WORKTREE_ALLOCATED | 唯一任务绑定、无已有占用冲突 |
| WORKTREE_ALLOCATED → CHANGES_OBSERVED | 对象/真实 diff 可读，所需范围明确 |
| CHANGES_OBSERVED → CANDIDATE_READY | 无未解决文本冲突，产生精确候选 |
| CANDIDATE_READY → VERIFIED | 必需分析完成，技术判定符合门禁规则 |
| VERIFIED → MERGE_ELIGIBLE | 可信控制面完成授权与 grant 校验 |
| MERGE_ELIGIBLE → MERGED | 原子写确认、回执绑定实际落地对象 |

异常分支：规则违规进入 BLOCKED，构造冲突进入 CONFLICT；绑定变化进入 STALE；必需覆盖不足为 INDETERMINATE；写结果未知进入 RECOVERY_REQUIRED。取消状态保留审计，不能抢占或撤回已发生的远端写入。重新运行创建新版本，不能把 STALE 原记录改成 VERIFIED。任务状态、技术 decision、运行 runStatus、执行 receipt 是不同维度。

## 5. 协议、证据与审计

共享现行 `guard.partme.ai/v1alpha1` 仅支持 GuardContract YAML / GuardFacts JSON / GuardReport JSON 和精确 `forbid_relation`，字段严格，规则 enforcement 为 `enforce/review/advise`，decision 为 `ALLOW/BLOCK/REQUIRE_APPROVAL`。事实 partial 导致 `BLOCK/INDETERMINATE`；complete 仅表示分析器声明的范围完成。GitGuard 适配尚不存在；OID CAS、操作授权和语义冲突不能靠现行 YAML 自动获得。

报告未签名，verify 是重算，不构成来源认证。可信控制面还需验证来源、摘要绑定、权限、时效和撤回。计划 `guard.integration/v1alpha1` 独立承载运行状态、调用绑定、摘要引用、分析覆盖、批准引用和诊断，不能给现行 GuardReport 添加字段。失败/取消不伪造 ALLOW，批准不能覆盖工具故障或不完整分析。

审计记录请求者/执行身份、runId/operationId、输入摘要、候选/目标前后 OID、grant 引用、状态变化及回执时间；批准与私钥/访问令牌分离，日志不保存凭据。保留撤销和失败记录，限制读取、保留期与删除权限；外部可认证存储是未来信任边界，日志文件本身不是防篡改证明。

## 6. 安全决策与生态迁移

- GG-ADR-001：worktree/index/commit/final candidate 不可互换；完整结果必须绑定检查对象。
- GG-ADR-002：不可变绑定和原子目标条件同时防止证据错用与 TOCTOU。
- GG-ADR-003：图谱 unknown 不转为安全；必需分析缺失不能靠批准放行。
- GG-ADR-004：独立 gitflow-plugin 仅为未核验兼容目标；固定版本审查后做差分，迁移时保持单一规则所有权。
- GG-ADR-005：Hook 非强制边界；独立 CI/受保护平台负责准入。
- GG-ADR-006：未知写结果先对账，不自动 force/reset/删历史。
- GG-ADR-007：分析与执行分权；grant 来自可信控制面，Agent 不能自签。

不宣称已验证外部插件的 Python 实现、配置格式、apply 或退出码。既有 CodeGuard/其他插件的 commit/push 反馈也仅为需适配的局部证据。可通过版本化 GitFlow、GitHub、GitLab、CodeGraph 适配器扩展，但不得在契约中运行不可信策略脚本或绕过现行协议严格字段。

实施端口、故障矩阵、验收及未决项见[技术方案](technical-design.md)。
