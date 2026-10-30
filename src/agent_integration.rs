/// 返回当前二进制真实接入能力；不替代专业领域的运行结果。
pub(crate) fn tool() -> guard_install::Tool {
    let mut tool = guard_install::Tool::new(
        "gitguard",
        env!("CARGO_PKG_VERSION"),
        "运行时接线引导（格式版本 1）。\n<!-- owner: gitguard-runtime-install/v1 -->\n- 本区块仅管理运行时接线与 XML 引用，不复制规则。先读取 git-common-dir/gitflow.xml，不得假定统一 GitFlow。使用 policy check 或 gitguard_policy_check 只读检查分支命名、合并方向和待提交消息/身份规则；必须显式提供可信 gitflow_root 与 python 绝对路径。allow 仅表示本次策略检查允许，不授权 Git 写入，不证明技术质量或最终合并候选通过。",
        &["policy check"],
    );
    tool.context = Some(crate::gitflow_context::read);
    tool.mcp_tools = crate::policy_check::TOOLS;
    tool
}
