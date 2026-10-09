# GitGuard — 技术实施方案

> 当前分支已有受限本地实现，13/28任务已独立验收；准确状态见[实施进度](implementation-progress.md)。历史main e03b5fd仅有文档，但不代表当前树。当前有Rust库/check CLI、真实Git候选、GE产物及本地history/CAS；没有生产身份provider、已批准非空基线、托管准入或可用写执行器。本文保留长期设计，未列入实现进度的能力仍为目标。测试authority/graph fixture不等于生产服务。

## 1. 当前技术与扩展目录

当前为Rust2024单crate、Serde、受限同步Git子进程与真实GuardEngine源码依赖。没有引入Clap/Tokio/git2。Rust声明下限1.90；固定runner实际为/usr/bin/git2.47.3，支持SHA1/SHA256及实际merge-tree能力测试。工具链实测矩阵和资源/文件系统边界见[观察ADR](git-observation-adr.md)。

~~~text
# 当前实现（按文件/模块组织）
src/{git,subject,scope,worktree,candidate,conflicts,preflight,evidence,execution,cli}
tests/  schemas/  fixtures/candidate/  examples/export_evidence.rs
adapters/gitflow/   # 固定版本调查，尚无启用的adapter
openspec/
# 后续目标：github/gitlab/codegraph/MCP，grant/apply/intent/reconcile
~~~

首期单 crate、显式接口、只读观察；Git 解析和领域审批保留在 GitGuard/控制面，不塞入 GuardEngine。

## 2. 领域数据契约（建议，不是现行线格式）

| 对象 | 必要内容及不变量 |
|---|---|
| RepositoryRef | 可信 repoId、对象算法、canonical commonGitDir、worktree 路径、受控远端映射；不得仅用可变 URL 认证 |
| TaskChangeContract | taskId、requirementIds、范围规则、基线 immutable ref/digest、目标策略及审批引用；批准记录外部认证 |
| WorktreeBinding | worktreeId、taskId、repoId、分支 ref、创建版本/租约；目录复用产生新 ID |
| ChangeSet | 比较对象类型、base/head OID、增删改/rename 两端/mode/symlink/submodule；稳定顺序与非 UTF-8 路径策略 |
| SymbolImpact | provider/consumer、符号/Schema/事件版本、分析器与索引版本、覆盖及 unknown；不承诺无冲突 |
| MergeCandidate | 候选提交 OID、tree OID、base/head、targetRef/expectedTargetOid、mergeGroupId、组成员快照、构造模式、算法、契约/基线摘要 |
| OperationGrant | issuer/actor/action、repo、精确对象、candidate、expected target、有效期、nonce/operationId、审批引用和可验证认证材料 |
| DomainResult | runId、绑定、输入摘要、领域发现、分析器/覆盖、Git 版本与观察来源；经适配器产生合法 GuardFacts |
| OperationReceipt | operationId、请求摘要、执行身份、前后 OID、平台回执、确定/未知状态、对账记录；与技术 GuardReport 分离 |

OID、摘要、策略 revision、schema version、crate semver 是不同字段和生命周期。`candidateOid` 指实际验证提交；不能将 tree OID 填入提交字段。规范化路径不跟随逃逸符号链接；策略匹配使用 repo-relative 路径，保留原始字节以便无损报告，展示转义不能改变判定。规范化摘要方案需统一版本与测试向量，不能散列未规范 JSON 后假定跨实现一致。

### 2.1 跨守卫协议

现行 `guard.partme.ai/v1alpha1` 严格字段，仅 GuardContract YAML、GuardFacts/GuardReport JSON；规则仅精确 `forbid_relation`。GitGuard 将已计算的领域关系映射到允许的 facts，不扩充当前 parser。OID 比较、候选构造、CAS、授权及图谱计算由领域实现。partial facts 为 `BLOCK/INDETERMINATE`；complete 是声明范围的完成度，不是全仓库正确性证明。

实际源码集成已使用独立[集成契约](integration-contract.md)的 `guard.integration/v1alpha1` GuardRunEnvelope；GuardEngine新增integration接口解析它，原生v1alpha1格式保持不变。它分离 completed/error/cancelled、可空 decision、调用绑定、contract/facts/report 摘要、分析器/覆盖、审批引用和 diagnostics。候选/基线/组、规则、分析器、覆盖、批准基线 revision 改变或批准到期/撤回均使相关门禁结果失效。unsigned report 的 verify 只重算，可信来源及授权由控制面验证。

## 3. 端口、CLI 与输出

| 目标端口 | 输入 → 输出 | 副作用/失败边界 |
|---|---|---|
| RepoDiscover | 显式路径 → RepositoryRef | 只读；坏仓库、跨边界、算法不支持为错误 |
| BranchPolicyCheck / DiffScopeScan | 绑定+范围+对象 → 观察与覆盖 | 只读；缺对象不自动 fetch |
| MergeCandidatePreview | 固定 base/head/策略 → 候选预览 | 仅受控临时对象库；不改用户 refs/index |
| ConflictAnalyze | 独立任务快照+版本化索引 → 风险/unknown | 只读；必需索引缺失不放行 |
| EvidenceQuery | 完整绑定+规则/分析器版本 → 匹配记录 | 不按 branch 名返回可冒用的旧结果 |
| WorktreeCreate / BranchCreate | 明确对象+对应 action grant → 回执 | 独立显式写，名称占用不能覆盖 |
| ProtectedPush / ApplyMerge | 候选+grant+幂等请求 → 回执 | 受保护原子执行；未知转对账 |

当前可运行入口为`gitguard check < request.json`，详见[CLI契约](git-engine-cli-contract.md)。以下仍是**不可运行的未来命令**，不能当作当前二进制接口：

~~~sh
gitguard doctor --project .
gitguard scope check --task TASK-104 --base main --head HEAD
gitguard conflict analyze --task TASK-104
gitguard merge preview --target main --head HEAD
gitguard merge apply --candidate candidate.json --grant signed-grant.json
~~~

最后一条只适用于未来受信执行身份；文件名 `signed-grant.json` 不证明其签名有效，更不能通过 `approved=true` 自授权限。ref 参数解析为固定对象后才能计算/存储结果；运行中不重新解释 HEAD。

当前check已实现stdout单个bundle JSON、stderr脱敏诊断及0 ALLOW、2 BLOCK、3 REQUIRE_APPROVAL、4输入/运行/验证错误。绑定前错误无envelope；绑定后error/cancelled保持null decision。当前明确不支持`--report`，拒绝未知参数，不复用旧文件。错误单独封装，不能给 v1alpha1 GuardReport 添加运行错误字段。写命令需要独立 receipt/退出语义；自动化不能以 check 的退出码推断写入成功。旧插件退出码需核验并保留，适配层显式转换，不静默改原 CLI。

## 4. 候选构造与并发控制算法

1. 注册一次调用：解析可信 repo/task/worktree/requirement 映射，固定基线摘要、scope revision、target OID、source head 和运行 ID。脏 worktree 或 index 检查使用独立 subject 类型；不得冒充提交检查。
2. 确认输入对象均存在；缺对象返回明确诊断。网络同步是单独授权步骤，记录来源/时间，不隐藏于 read-only check。
3. 从固定对象计算 diff，在受控临时存储构造 candidate。文本冲突返回 CONFLICT；对象/进程失败返回 error。队列场景优先使用平台提供的实际 candidate，不重造“看起来相同”的提交。
4. 对精确候选运行必需的 Spec/Arch/Code/Test/Git 检查；全部结果绑定同一 candidate/base/group 和相应基线、输入摘要。tree 一样但 commit/parents 不同也不能默认复用。
5. 结果 append-only 存储，key 包含不可变绑定、规则/基线和分析器/coverage digest；相同输入可复用计算，但仍需重验授权时效。更新任务当前指针时 CAS 比较 generation；迟到结果仅存为历史。
6. 门禁重读当前绑定与批准记录。技术满足不直接写入；精确 grant 交给分权执行器。目标改变返回 STALE，构造新候选重验，不“顺手 rebase 后推送”。

并行任务工作区/输出隔离；每个 worktree 的观察和共享 ref mutation 使用明确锁/租约，获取顺序稳定避免死锁。取消会停止后续调度并回收本运行临时资源；不能删除另一租约的资源。取消/重复 webhook 都保留独立审计，事件去重不代替对象新鲜度检查。

## 5. Grant、执行器和幂等恢复（未来目标；当前写端全部拒绝）

执行前逐项验证签发者信任链、actor、action、repo、候选/目标绑定、not-before/expiry、撤回、审批 scope、请求摘要、nonce 与 operationId；时钟不可靠时拒绝授权。生产验证器不得接受 Agent 自建 issuer、公钥或 PR 中的信任根。不同动作分别授予权限，禁止通配 ref 或把 worktree-create 许可升级为 merge。

幂等存储在写前持久化 intent：同 operationId+同请求返回原状态/回执；同 operationId+不同摘要拒绝。多执行器通过共享唯一键和状态 CAS 争抢一次执行资格，不能仅靠内存锁。最终 ref 更新必须由平台原子前置条件保护；“恰好一次”不靠 HTTP 重试承诺实现。

| 故障 | 目标结果与恢复 |
|---|---|
| 非法输入/仓库缺失/不支持算法 | error，检查退出 4；诊断明确，不生成 ALLOW |
| 缺所需对象、分析器超时/崩溃 | error；若产生合法 partial facts，则另有 BLOCK/INDETERMINATE 报告；均不满足门禁 |
| 越界路径/受保护策略变化 | completed + BLOCK，修订变更或按正式流程修改可信范围 |
| advisory 图谱缺失 | 记录 unknown/覆盖，不能宣传无冲突；必需图谱则阻断 |
| candidate/base/group 或 ruleset 漂移 | STALE；新绑定重验，旧记录保留 |
| grant 过期/撤回/错误动作或 actor | 拒绝写，重新取得真实授权；不能批准覆盖分析失败 |
| 原子目标条件失败 | 未执行的冲突/STALE；读取新目标后重新验证 |
| 写后响应丢失、进程崩溃 | RECOVERY_REQUIRED；通过操作标识/平台回执与远端对象对账，禁止直接重放 |
| 取消与远端写并发 | 不能假设取消成功；同样对账实际远端结果 |

对账不能只看当前 ref 等于候选：目标可能已再次前进。先查平台 operation receipt，再检查可验证的对象/历史关系；仍不确定就保留未知并由受信操作员处理。只有确认写未发生、旧 intent 可安全重试且授权仍有效时才能重试；不自动强推、回退目标、删除用户分支或补偿性 reset。

## 6. Git 与平台集成、安全边界

Git 调用以固定 executable 和 argv 执行，路径参数使用选项边界，解析 NUL 分隔输出；候选只读探测包括 `status --porcelain=v2 -z`、`diff --raw -z`、`rev-parse`，构造能力包括 `merge-tree`，具体 flags 需真实版本测试。禁用外部 diff/textconv、隐式 hooks、分页器、交互凭据询问等可执行入口；隔离不可信 repo config、环境变量、credential helper、URL rewrite 与 alternate object 路径。不要 blanket 放宽 safe.directory。对进程树、输出、对象/文件大小、递归深度和时间设上限；诊断脱敏。

GitHub 适配目标：保护规则、required checks、merge queue 实际 merge_group 候选、CODEOWNERS 与独立 CI 身份。GitLab 保护分支、流水线、审批及候选机制需单独能力探测，不能假设字段等价。上述均为待验证平台集成，尚无本地适配实现。平台若不能保证受检候选与写入对象一致，拒绝声明强制门禁完成。

只读分析默认无外部副作用、无写凭据、无不可信策略脚本。候选代码执行隔离在容器/VM 和受限网络中；worktree 本身不足以隔离权限。保护策略/授权 trust root 从独立受保护来源读取。扩展 adapter 必须声明能力、版本、覆盖、超时、权限、失败语义和兼容性测试。

## 7. 分阶段目标（部分本地任务已验收，不能按整阶段宣称完成）

| Wave | 交付 | 可测量完成条件 |
|---|---|---|
| G0 | 仓库发现、对象/subject、scope 和候选模型 | 临时真实 Git 仓库覆盖 SHA 算法、坏仓库、缺对象、非 UTF-8/rename/mode/symlink/submodule；每次只读前后 refs/index/worktree hash 不变 |
| G1 | 固定版本 GitFlow adapter、范围执行 | 先检查外部 provider/config/退出码；合法、违规、unknown、恢复用例双跑；所有差异记录并裁决，无静默规则所有权冲突 |
| G2 | 精确候选、并发证据、grant、对账 | 双 worktree/多需求/晚到结果/重复请求/target 漂移/授权过期撤回/写响应丢失均有确定断言；不得合入未检查 OID |
| G3 | 一种受保护托管平台及队列 | 集成环境中 Hook 绕过仍不能合入；组重排/目标推进使旧检查失效；实际落地对象对应受检候选，失败不得误报成功 |
| G4 | 语义图谱、第二平台、MCP/API | provider/consumer 跨文件风险与不完整索引均有覆盖；平台能力差异被显式拒绝或映射；每端口复用同授权与绑定校验 |

后续写端验收计划使用临时bare远端、多个worktree、受控并发和故障注入；断网/进程崩溃覆盖 intent 写前、远端写前、远端写后和回执写前。检查规则/CI 被候选恶意替换、错误 issuer、跨 repo grant、过期批准和 namespace collision。不得在真实用户分支上做 destructive reset/force push。当前真实Git/GE/CAS测试和完整日志另记实施进度；文档验证不替代运行验证。

## 8. 未决项与可逆默认

- 候选提交构造、merge/rebase/squash 策略与平台对象一致性：首期仅支持能提供精确候选与原子准入的方式，其余显式不支持。
- 批准服务、签名/认证格式、撤回和时间服务：首期使用单一可信控制面接口；未配置或不可验证则拒绝执行，不固定未经验证的密码学格式。
- 存储、租约、保留期与恢复服务：首期持久化追加记录和事务唯一键；后续扩展前做崩溃/多实例测试，不把本地 JSON 当分布式锁。
- 图谱覆盖阈值、路径大小写/字节编码、子模块递归范围：通过版本化 scope 明确；默认未知不作无冲突保证，必需范围缺失阻断。
- 旧插件已完成固定版本源码和只读执行调查，见[记录](../adapters/gitflow/compatibility.md)；opt-in映射、完整差分和迁移尚待实现，不宣称替代能力。


### 集成错误的绑定前置条件

上述目标 error/cancelled 信封只适用于调用身份、精确候选/基底、producer 与必查覆盖已经冻结的尝试。参数非法、仓库不可解析或绑定歧义等前置故障使用独立传输诊断和失败退出状态，不生成 GuardRunEnvelope，不伪造 OID 或空字段；当前各 CLI 的既有行为仍按本文事实表保留。详见[共享契约](integration-contract.md)。


## 当前持久化与信任边界

进程内CAS已按本地profile验收；另有Linux owner-private文件后端，使用稳定flock、实际GE状态机重放、原子rename及file/directory sync，并测试重启/两个进程/rename前后退出。其独立审查不等于实现第5节的操作intent。只在文档限定本机文件系统条件下支持，详情见[持久化ADR](durable-history-adr.md)。当前consume_bound在真实provider求值前冻结完整ReuseKey；未绑定或不同dependencies/config的消费结果不能记到新ticket。默认provider不可用，测试provider不授予生产权限。
