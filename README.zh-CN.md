# GitGuard

[English](README.md) · [简体中文](README.zh-CN.md)

**只读 Git 候选观察、冻结变更范围与绑定精确候选的证据。**

当前实现分支已有 Rust 库、`gitguard check` CLI、Schema 和真实 Git 测试，不再是纯文档项目。**13/28 个 OpenSpec 任务已按明确的本地 profile 独立验收**；新增工作仍按记录等待评审。准确边界见[实施进度](docs/implementation-progress.md)与[任务](openspec/changes/add-candidate-bound-git-governance/tasks.md)。

## 已实现的本地能力

- 真实 SHA-1/SHA-256 对象、linked worktree common directory、commit/tree/index/worktree 区分、完整不可变 OID 与字节安全路径范围，包括 rename 两端。
- 私有临时对象库中的双父合并预览，精确 candidate/base/group/member 绑定及任务/worktree 租约隔离。脏工作区、子模块内容和图谱覆盖缺失仍明确保留不确定性。
- 真实 GuardEngine 投影、有预算限制的求值及产物验证。`gitguard check` 从 stdin 读取严格 JSON 请求，stdout 返回本次 bundle：0 ALLOW、2 BLOCK、3 REQUIRE_APPROVAL、4 失败。没有 `--report` 或可变 ref 便捷命令。
- 显式 trust provider 端口、完整复用键绑定的消费结果、进程内 append-only 历史/CAS。独立的受限 Linux 本地文件后端已测试重启与多进程行为，其评审状态另行登记。历史资格不是当前授权。

测试中的语义图谱和身份/批准 provider 明确属于 fixture。**没有生产身份 provider、已批准非空基线、托管平台强制准入、MCP 服务、远端 push/merge 或 operation-intent 执行器。** 所有写操作仍拒绝，启用 feature 也不开放。ALLOW 是技术结果，不是合并许可；批准不能修复 partial/error，也不能改写 REQUIRE_APPROVAL。

观察目前要求受控本地 checkout；恶意并发目录替换与完整 OS 资源隔离尚未证明。repo label 由调用者指定，不以 origin URL 认证。支持契约包含[观察限制](docs/git-observation-adr.md)、[历史限制](docs/local-evidence-store-adr.md)和[Linux 持久化限制](docs/durable-history-adr.md)。

## 构建与运行

开发依赖相邻 `../guardengine` 源码及 Cargo.lock，尚非独立发布包。声明 Rust 下限为 1.90；实际验证的工具链/Git 矩阵见观察 ADR。

```sh
cargo test --locked
cargo run --locked -- check < request.json
```

按[实际 CLI 契约](docs/git-engine-cli-contract.md)构造 `request.json`：使用真实不可变 candidate/base OID，以及从 `FrozenPolicy` 导出的范围策略摘要。[CLI 测试](tests/cli_outcomes.rs)会构造真实请求。尚无 `scope check`、`conflict analyze` 或 `merge apply` 命令。

生成完整真实对象的 producer fixture 及精确产物字节：

```sh
cargo run --locked --example export_evidence -- /absolute/path/to/new-golden-directory
```

该示例创建临时仓库，运行真实 producer，导出 contract/facts/report/envelope/domain、完整 bundle 及源 Git bundle；不把 fixture 认证成生产证据。

## 协议与待实现工作

原生 `guard.partme.ai/v1alpha1` 保持严格 GuardContract/GuardFacts/GuardReport 与精确 `forbid_relation` 不变。实际集成使用独立 `guard.integration/v1alpha1` envelope 和资格端口；候选字段保留在 GitGuard 的 `gitguard.candidate/v1alpha2` 领域对象中。摘要及重算不证明签发者身份。

已对固定提交的官方 gitflow-plugin 做源码及只读实际调查，见[兼容性记录](adapters/gitflow/compatibility.md)。其退出语义不同；尚未启用 opt-in adapter 或宣称替代，原生接口未改动。

待办包括受保护基线来源、生产 provider 接入、更完整的进程/文件系统加固、Gitflow 差分适配、版本化 MCP/API、托管保护，以及另行审查的写授权/恢复。[架构](docs/architecture.md)和[技术设计](docs/technical-design.md)包含这些长期目标，未来能力不代表现有实现。[集成契约](docs/integration-contract.md)与[依赖路线图](openspec/guard-roadmap.md)说明跨仓边界。
