---
name: gitguard-policy-check
description: 在检查分支命名、合并方向或提交消息时复用已激活 GitFlow 策略；只读检查，不创建分支、合并或提交，不授予质量或发布批准。
license: Apache-2.0
compatibility: GitGuard CLI/MCP、显式可信 gitflow-plugin 与 Python；不安装或初始化工具。
---

# 激活 Git 策略检查

1. 确认目标项目和用户选择的可信 GitFlow 插件、Python 绝对路径；不从项目自报配置或 receipt 中选择可执行文件。不假定固定的 GitFlow 角色或合并方向。
2. 使用 `gitguard policy check --project PROJECT --gitflow-root TRUSTED_PLUGIN --python PYTHON --action branch --branch PROPOSED_BRANCH` 检查拟用分支名。更换为 `--action merge --source SOURCE --target TARGET` 检查已有本地分支合并方向，或 `--action commit --message MESSAGE` 检查当前分支和拟用提交消息/身份。
3. MCP `gitguard_policy_check` 使用 project/gitflow_root/python/action，加对应 branch、source/target 或 message 参数；CLI 和 MCP 复用同一判定。
4. 依据原激活策略给出的规则和原因修改拟用参数，再重新检查。缺激活、漂移、未知引用、适配失败或不能安全读取工作树时按 unverified 处理；不要通过改策略、清空检查或伪造结果掩盖违规。
5. decision=allow 仅表示当前领域策略通过；quality_decision 仍 not_evaluated。真实 Git 写入、合入候选新鲜度、CI 与审批另行核验，本技能不执行。

Git 查询可能受本机配置影响。工具对有效可执行 clean/process filters 保守返回 unverified，不能为了获得 allow 自动改用户配置；修复应基于可信隔离环境或显式授权。提交作者检查需保留实际 GIT_AUTHOR/GIT_COMMITTER 环境，不以本地默认身份替代。

CLI 退出码：0 策略允许，2 策略拒绝，3 未验证，4 参数或执行错误。安装成功与上下文提示不产生策略、技术质量或审批通过结论。
