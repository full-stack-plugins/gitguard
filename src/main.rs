mod agent_integration;
mod gitflow_context;
mod policy_check;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("policy")
        && args.get(1).map(String::as_str) == Some("check")
    {
        return policy_check::cli(&args[2..]);
    }
    if let Some(code) = guard_install::dispatch(&agent_integration::tool(), &args) {
        return code;
    }
    match args.first().map(String::as_str) {
        Some("--version") if args.len() == 1 => {
            println!("gitguard {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("--help" | "-h") | None => {
            println!(
                "gitguard: policy check --project ABS --gitflow-root TRUSTED_ABS --python ABS --action branch|merge|commit [--branch NAME | --source NAME --target NAME | --message TEXT]; install; uninstall; context; instructions render|sync; doctor --integration; serve --mcp\n策略检查只读，allow 不代表质量通过或批准写入。"
            );
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("unsupported command; use --help for implemented capabilities");
            ExitCode::from(4)
        }
    }
}
