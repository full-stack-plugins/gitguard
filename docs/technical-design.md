# GitGuard — 技术实施方案

> 目标 V0.1。当前 GitGuard 是新仓库，尚无独立 CLI；GitFlow 的行为/文档在单独插件仓库，不能把计划标记为已实现。

## 1. 技术栈与目录

Rust 2024、Serde、Clap、Tokio/进程限制、Git CLI（Git 2.41+ 优先）、GuardEngine SDK。Git2/libgit2 可考虑为只读细粒度分析补充，但 **Git push/merge/签名语义必须与真实 Git/托管平台一致**；不要自造一套 Git 对象数据库。

~~~text
gitguard/
├── src/{git,subject,branch,scope,worktree,diff,conflicts,merge,policy,evidence,cli}.rs
├── adapters/{gitflow,github,gitlab,codegraph}/
├── schemas/{change-contract,git-facts,merge-candidate,operation-receipt}/
├── fixtures/{clean,forbidden,rename,mode,drift,conflict,recovery}/
├── docs/
└── openspec/
~~~

首版优先单 crate + 受控 adapter；不把 GitGuard 的规则直接放进 GuardEngine 的通用引擎中。

## 2. 数据与版本模型

- RepositoryRef：repo id/url、gitDir/worktree、SHA 算法与权限上下文。
- TaskChangeContract：taskId、baseOid、expectedTargetOid、allowedPaths、allowedModules、protectedContracts、approvalDigest。
- ChangeSet：added/modified/deleted/renamed/mode/submodule 路径与实际 Git 对象标识。
- SymbolImpact：符号版本、provider/consumer、索引版本、覆盖与未知关系（可选）。
- MergeCandidate：base/head/targetOid/mergeTreeOid、规则摘要与检查集合。
- OperationGrant：绑定 actor、action、repo、candidate、targetOid、有效期、幂等键及批准者的独立授权（由可信控制面签发，不由 Agent 产生）。
- GitEvidence：gitVersion、sourceSnapshot、检查器 identity、diagnostics、completion 和 operationId。

推荐把更详细的 Git 信息封装为本域结果，并通过 Guard Protocol v1alpha1 的 GuardFacts/GuardReport 共享可检查的关系；当前 `forbid_relation` 无法原生完成基于 OID 的 CAS 或语义冲突计算，这些属于 GitGuard 代码与受控执行端，而非“写 YAML 就能有”。

## 3. 检查/执行端口

读端口：RepoDiscover、BranchPolicyCheck、DiffScopeScan、MergeCandidatePreview、EvidenceQuery。写端口：WorktreeCreate、BranchCreate、ProtectedPush、ApplyMerge；每项写操作需独立授权、精确对象和预览。仅预览不得 fetch 或修改索引；显式 apply 的用户约束必须继承 gitflow-plugin 语义。

目标 CLI（尚不存在）：

~~~sh
gitguard doctor --project .
gitguard scope check --task TASK-104 --base main --head HEAD
gitguard conflict analyze --task TASK-104
gitguard merge preview --target main --head HEAD
gitguard merge apply --candidate candidate.json --grant signed-grant.json
~~~

最后一条为未来可信执行器接口，绝不允许 CLI 参数自行设 `approved=true` 即触发合入。

## 4. 原生 Git / GitHub / GitLab 对接

- 本地：优先使用 `git status --porcelain=v2 -z`、`git diff --raw -z`、`git rev-parse`、`git merge-tree` 等有稳定协议的 argv，避免 shell 注入；支持 SHA1/SHA256 仓库识别。
- GitHub：branch rulesets、必需 CI、merge queue 的 merge_group 候选验证、CODEOWNERS、不可绕过策略由组织管理员配置并实际核验。
- GitLab：保护分支、流水线、审批机制按原生 API 单独实现，不假设两平台字段对等。
- CodeGraph：符号分析按版本及覆盖附加，而非核心阻断的唯一来源。
- 信任：不把 PR 中可改的 workflow/规则文件作为最终强制策略来源。

## 5. 故障恢复、幂等和隔离

每个写操作存储 operationId、预期目标版本与命令 receipt。超时、断连、进程崩溃、unknown push 结果时先 read-only 查询远端 refs/commit，再决定是否重试；严禁自动 force、回滚已有用户变更或创建双重分支。隔离的工作树仍共享本机凭据和 .git 元数据，需要容器/VM/权限沙箱来隔离不可信代码执行。

## 6. Waves 与测试

| Wave | 目标 | 验收 |
|---|---|---|
| G0 | Git 仓库发现、branch、scope、精确候选数据模型 | 无副作用读取和坏仓库诊断 |
| G1 | gitflow-plugin adapter、变更路径与受信范围 | 兼容原 GF 规则、rename/mode/越界 |
| G2 | 最终 merge tree 与目标漂移、受控操作与对账 | 模拟并行 push、断连/重放/过期拒绝 |
| G3 | GitHub 受保护分支、CI、merge_group | 绕过 Hook 无法服务器合入 |
| G4 | CodeGraph 语义冲突、GitLab、MCP/API | 不完整索引保留 unknown，跨平台回归 |

测试必须使用真实临时 Git bare 远端、并发 target ref 更新、多个 Worktree 和被故意破坏的检查脚本。验收不允许依赖对真实用户分支的 destructive reset/force push。
