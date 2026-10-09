# GitGuard — 技术实施方案

> 全文为待实现设计。检查基线 main `e03b5fd06d8b3d81d4bbbd11ff0fb70bea00d485`，2026-10-09；实际树仅包含双语 README、`docs/architecture.md` 与本文。没有源码/测试/构建清单/配置/Schema/OpenSpec，因此不存在可运行命令、已通过测试或 OpenSpec 校验结果。架构约束见[架构](architecture.md)，共享字段以[集成草案](integration-contract.md)为准。

## 1. 技术选型与拟议目录

候选技术为 Rust 2024、Serde、Clap、受限子进程执行器及 GuardEngine 集成；是否引入 Tokio 随并发实现确定。Git CLI 优先于自行实现 Git 对象数据库；Git2/libgit2 可作只读补充。Git 2.41+ 是初始兼容测试目标而非已验证最低版本，具体 merge-tree 功能必须能力探测。真实 push/签名/平台合并语义必须做集成验证。

~~~text
# 拟议目录；当前均不存在（docs 除外）
src/{git,subject,branch,scope,worktree,diff,conflicts,merge,policy,evidence,cli}.rs
adapters/{gitflow,github,gitlab,codegraph}/
schemas/{change-contract,git-domain-result,merge-candidate,operation-receipt}/
fixtures/{clean,forbidden,rename,mode,drift,conflict,recovery}/
openspec/
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

计划 `guard.integration/v1alpha1` GuardRunEnvelope 位于独立[集成契约](integration-contract.md)，现有引擎不解析。它分离 completed/error/cancelled、可空 decision、调用绑定、contract/facts/report 摘要、分析器/覆盖、审批引用和 diagnostics。候选/基线/组、规则、分析器、覆盖、批准基线 revision 改变或批准到期/撤回均使相关门禁结果失效。unsigned report 的 verify 只重算，可信来源及授权由控制面验证。

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

以下全部为**不可运行的拟议命令**；仓库没有二进制。参数命名尚未冻结：

~~~sh
gitguard doctor --project .
gitguard scope check --task TASK-104 --base main --head HEAD
gitguard conflict analyze --task TASK-104
gitguard merge preview --target main --head HEAD
gitguard merge apply --candidate candidate.json --grant signed-grant.json
~~~

最后一条只适用于未来受信执行身份；文件名 `signed-grant.json` 不证明其签名有效，更不能通过 `approved=true` 自授权限。ref 参数解析为固定对象后才能计算/存储结果；运行中不重新解释 HEAD。

未来检查命令目标：stdout 单个机器可读 JSON，stderr 脱敏诊断；0 ALLOW、2 BLOCK、3 REQUIRE_APPROVAL、4 输入/运行/验证错误。GitGuard 当前无 stdout/`--report` 实现；是否增加 `--report` 必须另行设计，不能把其他 CLI 行为当成本项目事实。错误单独封装，不能给 v1alpha1 GuardReport 添加运行错误字段。写命令需要独立 receipt/退出语义；自动化不能以 check 的退出码推断写入成功。旧插件退出码需核验并保留，适配层显式转换，不静默改原 CLI。

## 4. 候选构造与并发控制算法

1. 注册一次调用：解析可信 repo/task/worktree/requirement 映射，固定基线摘要、scope revision、target OID、source head 和运行 ID。脏 worktree 或 index 检查使用独立 subject 类型；不得冒充提交检查。
2. 确认输入对象均存在；缺对象返回明确诊断。网络同步是单独授权步骤，记录来源/时间，不隐藏于 read-only check。
3. 从固定对象计算 diff，在受控临时存储构造 candidate。文本冲突返回 CONFLICT；对象/进程失败返回 error。队列场景优先使用平台提供的实际 candidate，不重造“看起来相同”的提交。
4. 对精确候选运行必需的 Spec/Arch/Code/Test/Git 检查；全部结果绑定同一 candidate/base/group 和相应基线、输入摘要。tree 一样但 commit/parents 不同也不能默认复用。
5. 结果 append-only 存储，key 包含不可变绑定、规则/基线和分析器/coverage digest；相同输入可复用计算，但仍需重验授权时效。更新任务当前指针时 CAS 比较 generation；迟到结果仅存为历史。
6. 门禁重读当前绑定与批准记录。技术满足不直接写入；精确 grant 交给分权执行器。目标改变返回 STALE，构造新候选重验，不“顺手 rebase 后推送”。

并行任务工作区/输出隔离；每个 worktree 的观察和共享 ref mutation 使用明确锁/租约，获取顺序稳定避免死锁。取消会停止后续调度并回收本运行临时资源；不能删除另一租约的资源。取消/重复 webhook 都保留独立审计，事件去重不代替对象新鲜度检查。

## 5. Grant、执行器和幂等恢复

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

## 7. 分阶段验收（全部待实施）

| Wave | 交付 | 可测量完成条件 |
|---|---|---|
| G0 | 仓库发现、对象/subject、scope 和候选模型 | 临时真实 Git 仓库覆盖 SHA 算法、坏仓库、缺对象、非 UTF-8/rename/mode/symlink/submodule；每次只读前后 refs/index/worktree hash 不变 |
| G1 | 固定版本 GitFlow adapter、范围执行 | 先检查外部 provider/config/退出码；合法、违规、unknown、恢复用例双跑；所有差异记录并裁决，无静默规则所有权冲突 |
| G2 | 精确候选、并发证据、grant、对账 | 双 worktree/多需求/晚到结果/重复请求/target 漂移/授权过期撤回/写响应丢失均有确定断言；不得合入未检查 OID |
| G3 | 一种受保护托管平台及队列 | 集成环境中 Hook 绕过仍不能合入；组重排/目标推进使旧检查失效；实际落地对象对应受检候选，失败不得误报成功 |
| G4 | 语义图谱、第二平台、MCP/API | provider/consumer 跨文件风险与不完整索引均有覆盖；平台能力差异被显式拒绝或映射；每端口复用同授权与绑定校验 |

测试使用临时 bare 远端、多个 worktree、受控并发和故障注入；断网/进程崩溃覆盖 intent 写前、远端写前、远端写后和回执写前。检查规则/CI 被候选恶意替换、错误 issuer、跨 repo grant、过期批准和 namespace collision。不得在真实用户分支上做 destructive reset/force push。文档验证只检查相对链接、双语覆盖和当前/规划一致性，不冒称实现测试通过。

## 8. 未决项与可逆默认

- 候选提交构造、merge/rebase/squash 策略与平台对象一致性：首期仅支持能提供精确候选与原子准入的方式，其余显式不支持。
- 批准服务、签名/认证格式、撤回和时间服务：首期使用单一可信控制面接口；未配置或不可验证则拒绝执行，不固定未经验证的密码学格式。
- 存储、租约、保留期与恢复服务：首期持久化追加记录和事务唯一键；后续扩展前做崩溃/多实例测试，不把本地 JSON 当分布式锁。
- 图谱覆盖阈值、路径大小写/字节编码、子模块递归范围：通过版本化 scope 明确；默认未知不作无冲突保证，必需范围缺失阻断。
- 旧插件迁移细节与退出码：外部项目尚未检查；保留兼容目标，先收集固定版本事实再冻结映射，不宣称已有替代能力。
