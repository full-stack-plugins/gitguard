# GitGuard policy check

规格事实源：`guardengine/openspec/changes/professional-guard-checks`。

本插件能力由仓库根的 Rust 二进制提供。已有安装器支持 Codex、Claude Code、Gemini CLI：`gitguard install --agents codex,claude,gemini --project <绝对路径> --bin-dir <绝对路径>`。请先使用 `--dry-run` 检查受管文件计划；安装不会激活 GitFlow 或授权 Git 写操作。

## 已实现的只读检查

显式选择可信 `gitflow-plugin` 源码和现有 Python 可执行文件，所有路径必须为绝对路径。适配器是被信任的可执行代码，不应指向待检查项目提供的任意脚本。

```sh
gitguard policy check --project /repo --gitflow-root /trusted/gitflow-plugin --python /usr/bin/python3 --action branch --branch feature/my-task
gitguard policy check --project /repo --gitflow-root /trusted/gitflow-plugin --python /usr/bin/python3 --action merge --source feature/my-task --target develop
gitguard policy check --project /repo --gitflow-root /trusted/gitflow-plugin --python /usr/bin/python3 --action commit --message 'feat: implement task'
```

- `branch`：通过原策略加载器与 `role_for` 解析候选命名，Git 自身验证分支名；不创建分支。
- `merge`：复用原 `service.gate`，要求存在本地源和目标分支，验证激活角色的合并方向；不执行 merge 或 fetch。
- `commit`：检查当前分支是否允许提交、消息模式及策略版本支持的作者规则；不创建提交。它检查待提交信息，不扫描历史提交。

策略来自原 `policy.load`，保留组织继承、激活摘要、工作树定位与漂移检测。没有激活记录、策略漂移、未知引用、适配器缺失或不兼容时返回 `unverified`，不会使用内置默认工作流代替。

stdout 是 JSON。`decision` 为 `allow`、`deny` 或 `unverified`；退出码依次为 0、2、3，非法参数或执行错误为 4。`quality_decision` 始终是 `not_evaluated`。`allow` 只表示当前策略检查允许，不能证明测试通过、服务端合并门禁生效、候选 OID 新鲜度或审批授权。

## MCP

`gitguard serve --mcp --project /repo` 暴露 `gitguard_policy_check`，参数为：

```json
{"project":"/repo","gitflow_root":"/trusted/gitflow-plugin","python":"/usr/bin/python3","action":"merge","source":"feature/my-task","target":"develop"}
```

每次领域调用必须显式传入可信适配器；与上下文服务启动参数分别校验。`action=branch` 只接受 `branch`；`action=merge` 只接受 `source,target`；`action=commit` 只接受 `message`。拒绝未知参数和 `apply`。CLI 与 MCP 共用 `policy_check::check`，返回相同判定对象。

## 验证

`tests/policy_check.py` 创建隔离的临时 Git 仓库和测试激活快照，检查负例→修正请求→复验、CLI/MCP 全 JSON 相等、保护分支、未知引用、缺失适配器、策略漂移和检查前后工作树状态。仅测试夹具写入自己的临时仓库。

Windows 本地开发将 `CARGO_HOME`、`CARGO_TARGET_DIR`、`TEMP`、`TMP` 设到 E 盘，构建后设置 `GITGUARD_BIN` 为二进制绝对路径，运行 `python -B tests/policy_check.py`。测试要求已有可信的兄弟 `gitflow-plugin` 检出；不安装依赖或激活真实目标仓库。

Portable package: root plugin.json and mcp.json register the already-built gitguard executable from PATH; skills/ contains the package-local usage skill. The package does not bundle or install the executable. Structure and stdio discovery are checked by guardengine/scripts/verify_guard_packages.py; actual host loading remains unverified.
