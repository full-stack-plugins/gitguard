# GitGuard — Git 守卫

已实现专业检查：`policy check --action branch|merge|commit` 与 MCP `gitguard_policy_check`，复用已激活 GitFlow 策略，全程只读。见[插件使用、可信边界与验证](plugin/README.md)。`allow` 仅表示策略允许，技术质量与审批授权仍未评估。

[English](README.md) · [简体中文](README.zh-CN.md)

GitGuard 面向 **AI 多智能体并行研发**，管控分支、Worktree、任务变更范围、候选代码、语义冲突、合入基线和受保护分支的安全性。

> **当前状态：源码已提供可构建的智能体接入 CLI 和只读 MCP。** 可通过显式可信适配器读取 GitFlow 生效规则；Git 写操作与服务端强制门禁尚未实现。

## 最重要的工程问题

Agent A 修改订单聚合，Agent B 修改支付服务，即使没有改同一个文件，也可能因共享 API 破坏兼容性。更危险的是：任务在旧 main 上检查通过，真正合并时 main 已变更。这不是一个 Git Hook 能单独保证的。

~~~text
任务批准范围 → 分支/变更实际检查 → 影响/冲突分析
                                      │
                            构造最终合并候选
                                      │
                       GuardEngine + 技术证据
                                      │
                     FlowGuard 许可 + 可信 Git 执行器
                                      │
                     受保护目标分支 / 合并队列
~~~

## 核心职责

- Branch/Worktree Guard：任务分支身份、来源、命名、生命周期。
- Change Scope Guard：允许的文件/模块/公共契约范围及真实 diff。
- Semantic Conflict Guard：不同任务对共享符号/API/Schema 的潜在影响；不完整图谱标 unknown。
- Merge/Baseline Guard：确认真实最终候选、最新目标版本、合并门禁与证据失效。
- Trusted Git Operations：校验操作对象和授权、原子目标引用条件、未知结果对账。

**GitGuard 的局部 PASS 不等于整个工程质量通过。** 它消费 SpecGuard/ArchGuard/CodeGuard/TestGuard 的相应证据，不能自行创建技术审批。GuardEngine 负责通用协议；FlowGuard 管理整个研发流程审批。

## 文档

[架构设计](docs/architecture.md) · [详细技术方案](docs/technical-design.md)

## 规划命令（尚不可运行）

~~~sh
gitguard scope check --task TASK-104 --base main --head HEAD
gitguard conflict analyze --task TASK-104
gitguard merge preview --target main --head HEAD
~~~

首次开发先复用现有 GitFlow 规则和兼容配置，建立精确 Git 候选及越权变更负例；随后完成真实 CI、受保护分支和并行语义冲突验证。不会因为“代码没有文本冲突”就断言合并安全。

## 智能体安装与提示注入

已接入共享安装器：`install`、`uninstall`、`context`、`instructions render|sync`、`doctor --integration`、`serve --mcp`。支持 Codex、Claude Code 和 Gemini CLI；按本工具职责生成独立标记区块。

```sh
cargo build --locked
gitguard install --agents codex,claude,gemini --project /absolute/project --bin-dir /absolute/bin --dry-run
gitguard context --project /absolute/project
```

构建后请先通过 `target/debug/工具名`（Windows 加 `.exe`）调用；上面的短命令假定程序已加入 PATH。指定 `--bin-dir` 会复制当前二进制，安装器不自动修改 PATH；去掉 `--dry-run` 才实际安装。需要同级 `../guardengine/crates/guard-install` 源码。

动态规则需显式 `--gitflow-root <可信插件路径> --python <已有Python可执行文件>`；CLI 查询与同步需重复参数，MCP 配置固定参数。候选漂移返回 unverified，未提供适配器返回 unconfigured。

[统一设计、宿主文件映射、更新、卸载和验证说明](https://github.com/full-stack-plugins/guardengine/blob/main/docs/agent-installation.md)。安装成功不代表宿主已加载或专业检查通过。


只读策略/上下文查询在 Windows/POSIX 都先检查有效 Git `filter.*.clean` 与 `filter.*.process` 配置。存在非空有效命令（含继承的 Git LFS）时，直接返回 `unverified` / `executable_filter_unverified`，不启动 filter，不删除配置，也不伪造干净状态。此保守限制需通过隔离环境或另行授权的工作流处理。Windows 清理仓库/配置路由环境变量，同时保留原生 `GIT_AUTHOR_*` / `GIT_COMMITTER_*` 身份变量；两平台均禁用可选索引刷新写入。这是本地配置检查，不是对并发修改仓库/配置的安全沙箱。
