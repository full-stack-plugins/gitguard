use serde_json::{Value, json};
use std::{collections::BTreeSet, path::Path, process::ExitCode};

pub(crate) const TOOLS: &[guard_install::DomainTool] = &[guard_install::DomainTool {
    name: "gitguard_policy_check",
    description: "Read-only active GitFlow branch naming, merge direction or commit policy check. Explicit trusted adapter required. Does not merge, commit or approve quality.",
    schema,
    call: check,
}];

fn schema() -> Value {
    json!({"type":"object","properties":{
        "project":{"type":"string"},"gitflow_root":{"type":"string"},"python":{"type":"string"},
        "action":{"type":"string","enum":["branch","merge","commit"]},
        "branch":{"type":"string"},"source":{"type":"string"},"target":{"type":"string"},"message":{"type":"string"}
    },"required":["project","gitflow_root","python","action"],"additionalProperties":false})
}

/// 校验显式参数并复用 GitFlow 激活策略，返回真实只读领域判定；无 Java 对应。
fn check(args: &Value) -> Result<Value, String> {
    let object = args.as_object().ok_or("arguments must be an object")?;
    let action = args["action"].as_str().ok_or("action required")?;
    let fields: &[&str] = match action {
        "branch" => &["branch"],
        "merge" => &["source", "target"],
        "commit" => &["message"],
        _ => return Err("action must be branch, merge or commit".into()),
    };
    let allowed: BTreeSet<&str> = ["project", "gitflow_root", "python", "action"]
        .into_iter()
        .chain(fields.iter().copied())
        .collect();
    for (key, value) in object {
        if !allowed.contains(key.as_str())
            || !value.is_string()
            || value.as_str().unwrap().len() > 16384
        {
            return Err(format!("invalid argument: {key}"));
        }
    }
    for field in fields {
        if !args[*field].is_string() {
            return Err(format!("{field} required"));
        }
    }
    let mut paths = Vec::new();
    for key in ["gitflow_root", "python", "project"] {
        let path = Path::new(args[key].as_str().ok_or(format!("{key} required"))?);
        if !path.is_absolute() {
            return Err(format!("{key} must be absolute"));
        }
        paths.push(path);
    }
    let mut result = crate::gitflow_context::invoke(paths[0], paths[1], paths[2], Some(args))?;
    if result.get("decision").is_none() {
        result["decision"] = json!("unverified");
    }
    result["quality_decision"] = json!("not_evaluated");
    Ok(result)
}

/// CLI 与 MCP 使用相同校验和执行函数；退出码 0/2/3/4 分别代表允许/拒绝/未验证/参数或执行错误。
pub(crate) fn cli(args: &[String]) -> ExitCode {
    let result = (|| {
        let mut values = serde_json::Map::new();
        for pair in args.chunks(2) {
            if pair.len() != 2 || !pair[0].starts_with("--") {
                return Err("options require values".into());
            }
            let key = pair[0][2..].replace('-', "_");
            if values.insert(key, json!(pair[1])).is_some() {
                return Err("duplicate option".into());
            }
        }
        check(&Value::Object(values))
    })();
    match result {
        Ok(value) => {
            println!("{value}");
            ExitCode::from(match value["decision"].as_str() {
                Some("allow") => 0,
                Some("deny") => 2,
                _ => 3,
            })
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(4)
        }
    }
}
