# OpenSpec structural validation — gitguard

日期：2026-10-09。范围：增量实施规划；没有产品代码改动。

## Tool and command

官方工具 `@fission-ai/openspec@1.14.1`；Node.js v24.19.0。依据 [官方安装说明](https://openspec.dev/docs) 与 npm registry 元数据（repository: Fission-AI/OpenSpec），在仓库外独立云端工具目录安装；无全局配置/项目依赖安装。禁用安装脚本和遥测，未初始化或覆盖仓库工具配置。

```sh
openspec validate add-candidate-bound-git-governance --strict --no-interactive --json
```

实际退出码：**0**；`valid: true`；`issues: []`。

新 change：[add-candidate-bound-git-governance](changes/add-candidate-bound-git-governance/proposal.md)。包含 3 个 capability、14 条 Requirement、23 个 Scenario、28 个未完成任务；已勾选任务为 0。proposal/design/specs/tasks 和 change 元数据均存在。

## Existing changes

检查基线无旧 OpenSpec change；本次新增为首个规划 change。

## Interpretation and scope

通过仅说明 OpenSpec 结构和新 delta 合规，不证明计划中的运行时、审批、签名、隔离、SDK 兼容或队列集成已实现。现有 GitHub CI 记录是对应源码提交的回归证据，也不能替代新任务的实现验收。

逐仓读取旧 change/config/源码依据，保留原任务归属；跨仓复审修正 SG-BASELINE 本地契约与生产认证的区分、TestGuard 队列生产者与 FlowGuard 消费者的顺序、以及 signed-attestation 仍需真实签发/验签证据的条件。完整顺序见[路线图](guard-roadmap.md)。

本轮另检查新文件相对链接、任务无虚假勾选、规范性 Scenario、文档差异与共享路线图副本。计划中的执行器、生产权限和供应商选择未启用；未决项保持为显式任务。
