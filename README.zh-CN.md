# GitGuard

[English](README.md) · [简体中文](README.zh-CN.md)

**面向 AI 原生研发的 Git 策略、变更范围、并行任务分析、精确合并候选与安全 Git 操作。**

> **当前只有文档。** 2026-10-09 检查 main 提交 `e03b5fd06d8b3d81d4bbbd11ff0fb70bea00d485`，仓库只有两份 README 和两份设计文档，没有 CLI、MCP 服务、可执行程序、构建清单、测试、Schema 或 OpenSpec 目录。下述组件、命令和阶段均为目标方案。独立的 [gitflow-plugin](https://github.com/full-stack-plugins/gitflow-plugin) 是**尚未核验的兼容目标**；本次没有检查其实现或行为。

## 问题与方案

两个 Agent 修改不同文件，仍可能破坏同一个 API；本地 Hook 通过后，目标分支也可能继续前进。GitGuard 计划把批准的变更范围、真实 Git 对象、隔离的任务观察结果和验证证据绑定到**受保护合并机制实际接纳的精确候选**。

~~~text
批准的需求基线 + 任务范围 + 观察到的目标 OID
                     ↓
          分支 / Worktree / 真实 Diff 检查
                     ↓
             并行任务语义影响预警
                     ↓
           不可变候选 / 合并队列组绑定
                     ↓
           领域事实 → GuardEngine 证据
                     ↓
         FlowGuard 门禁 + 认证的操作许可
                     ↓
          可信执行器 + 受保护的原子准入
~~~

## 边界与场景

GitGuard 负责仓库与提交身份、分支/Worktree 策略、真实变更范围、候选新鲜度和 Git 写安全。SpecGuard 定义需求批准含义，ArchGuard 负责架构，CodeGuard 负责代码策略，TestGuard 负责测试证据，FlowGuard 负责流程授权。GuardEngine 提供通用契约校验、中立规则求值和确定性证据计算，不签发批准或执行合并；六个守卫相互独立。

- **并行需求：** 每个任务/Worktree 运行隔离；共享文件、API、Schema 产生冲突观察。迟到结果不能覆盖新候选证据。
- **合并队列：** 校验实际队列候选、基线和组身份。PR HEAD 的证据不能授权不同候选；目标/队列组变化使相关结果失效。
- **受控写入：** 预览只读；独立获授权的执行器验证范围有限且有时效的许可及预期目标 OID。写结果未知时，先向远端对账再决定是否重试。

输入是可信任务/范围与基线引用、本地 Git 对象和明确目标快照；规划输出为领域观察、不可变候选引用、符合协议的证据和独立操作回执。`ALLOW` 是限定范围的技术决定，不是合并/发布权限。批准不能覆盖分析不完整或工具失败。

Worktree 只隔离目录，**不隔离安全权限**。准入必须由受保护分支、独立必需检查和可信执行身份强制实施。契约和 CI 策略来自受保护来源，不能信任待检候选自己修改的规则。

## 协议与交付状态

共享现行协议 `guard.partme.ai/v1alpha1` 只包含 GuardContract YAML、GuardFacts JSON、GuardReport JSON，规则为精确 `forbid_relation`，严格限制字段，支持 `enforce`/`review`/`advise`。它没有 Git 候选或授权字段；GitGuard 尚无适配器。验证仅重算未签名证据，不证明身份或授权。

规划的[集成契约](docs/integration-contract.md) 在现有线格式之外携带任务/需求/Worktree/候选绑定；草案 `guard.integration/v1alpha1` 不被当前引擎解析。未来检查命令目标退出码为 0 `ALLOW`、2 `BLOCK`、3 `REQUIRE_APPROVAL`、4 输入/运行/验证错误；GitGuard 当前均未实现。部分事实应返回 `BLOCK` 与 `INDETERMINATE`，不能放行。

## 文档

- [架构、边界、并发与信任决策](docs/architecture.md)
- [技术方案、接口、恢复与可衡量阶段](docs/technical-design.md)
- [共享集成契约（草案）](docs/integration-contract.md)
- [GuardEngine 协议](https://github.com/full-stack-plugins/guardengine/blob/main/docs/protocol.md)

## 规划 CLI — 尚不可运行

~~~sh
# 仅为设计示例；仓库没有 gitguard 程序。
gitguard scope check --task TASK-104 --base main --head HEAD
gitguard conflict analyze --task TASK-104
gitguard merge preview --target main --head HEAD
~~~

可变 ref 参数仅为便捷输入：未来实现必须一次解析为不可变 OID 并记录解析结果。首期建立只读观察与精确候选测试，再以固定 provider 版本和差分测试评估旧插件兼容性。只读检查不允许强推、破坏性 reset、删除分支、隐藏 fetch 或执行策略脚本。
