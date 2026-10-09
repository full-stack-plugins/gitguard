# GitGuard — Git 守卫

[English](README.md) · [简体中文](README.zh-CN.md)

GitGuard 面向 **AI 多智能体并行研发**，管控分支、Worktree、任务变更范围、候选代码、语义冲突、合入基线和受保护分支的安全性。

> **当前状态：架构与技术方案文档已完成；新的 GitGuard CLI、MCP 和服务端强制门禁尚未实现。** 已有能力继续保留在 [gitflow-plugin](https://github.com/full-stack-plugins/gitflow-plugin)，后续经差分验证再迁移。

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
