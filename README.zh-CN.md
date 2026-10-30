# GitGuard

[English](README.md) · [简体中文](README.zh-CN.md)

GitGuard 围绕项目 git-common-dir/gitflow.xml 守护分支规范，作为原生 Git 的本地辅助门禁，不替代 Git 客户端，不宣称远端不可绕过。

## 当前默认运行时

Rust 默认入口负责项目安装、AGENTS 引导、只读上下文、策略检查与 MCP；显式指定的 [GitGuard 插件](https://github.com/full-stack-plugins/gitguard-plugin) 提供八预设初始化及 Git/Agent 守护接线。

- Rust 项目 install 独占受收据保护的 AGENTS 块和 MCP 配置，拒绝覆盖未知或被用户修改的块。
- 插件 init 只选择 GitFlow 并初始化 XML，不读改 AGENTS；linked worktree 使用公共 Git 元数据目录共享规则。
- XML 是唯一运行时 GitFlow 规则来源，缺失或损坏时未验证，不回退旧 JSON。
- 配置初始化不等于 Hook 已启用。已有 hooks/manager 不覆盖，需显式接线；不保证拦截人工终端的全部 fast-forward/no-commit/no-verify 操作。
- 真实 Agent 宿主加载及业务 CI 可信 XML 分发仍未验收或未配置；候选 CI 通过不等于生产就绪。

## 构建与使用

按 `.github/workflows/agent-installation.yml` 固定提交在同级检出 GuardEngine 和插件。默认目标需要 Rust 1.85+。

```sh
cargo build --locked
./target/debug/gitguard --help
./target/debug/gitguard install --agents codex --project /absolute/project --gitflow-root /absolute/gitguard-plugin --python /absolute/python --dry-run
python /absolute/gitguard-plugin/scripts/gitflow.py init /absolute/project --profile github-flow --apply --json
./target/debug/gitguard policy check --project /absolute/project --gitflow-root /absolute/gitguard-plugin --python /absolute/python --action merge --source feature/example --target main
```

Windows 使用 .exe。仅在有意安装到目标项目时去掉 --dry-run。插件的 install --bin-dir 只是启动器安装，与 Rust 项目安装不同。allow 只是当前策略判断，不等于质量审批或 Git 写授权。

其他当前命令：uninstall、context、instructions render|sync、doctor --integration、serve --mcp。开发过程继续使用原生 Git。

## 保留旧候选治理实现

旧 roadmap 分支完整保留在 [compat/candidate-governance](compat/candidate-governance/COMPATIBILITY.md)，含源码、CLI、测试、夹具、文档和示例。它是独立构建的 Linux 兼容目标，保留原固定 GuardEngine API 依赖和 Rust 1.90 下限，不参与默认三平台运行时安装或调用。历史 JSON 观测适配器不是当前策略权威；特权执行仍默认关闭，旧测试由独立工作流显式执行。

见[分支整合说明](docs/branch-integration.md)、[安装职责合同](docs/xml-installation-fixture.md)、[配套验证](docs/xml-adapter-verification.md)及[插件信任边界](plugin/README.md)。
