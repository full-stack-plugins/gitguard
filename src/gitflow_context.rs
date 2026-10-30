use guard_install::Options;
use serde_json::{Value, json};
use std::{
    io::Read,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

/// 读取用户显式选择的 GitFlow 生效规则；参数含目标项目与适配器路径，返回带覆盖状态的上下文。
pub(crate) fn read(options: &Options) -> Result<Value, String> {
    let (Some(root), Some(python)) = (&options.gitflow_root, &options.python) else {
        return Ok(
            json!({"status":"unconfigured","reason":"Select an existing trusted GitFlow adapter with --gitflow-root and --python; no workflow is assumed.","quality_decision":"not_evaluated"}),
        );
    };
    let value = invoke(root, python, &options.project, None)?;
    Ok(reference_context(value))
}

/// Export only a versioned reference, never a second policy inside AGENTS.md.
/// Domain policy checks still use invoke directly with the complete XML policy.
fn reference_context(value: Value) -> Value {
    let status = value["status"].as_str().unwrap_or("unverified");
    if status != "active" {
        return json!({"status":if status == "unconfigured" { "unconfigured" } else { "unverified" },
            "code":value["code"], "reason":value["reason"], "quality_decision":"not_evaluated",
            "reference_schema_version":1, "policy_location":"git-common-dir/gitflow.xml"});
    }
    let profile = value["profile"].as_str().unwrap_or("");
    let digest = value["policy_sha256"].as_str().unwrap_or("");
    if profile.is_empty()
        || profile.len() > 64
        || !profile
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        || value["revision"].as_u64().unwrap_or(0) == 0
        || digest.len() != 64
        || !digest
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return json!({"status":"unverified", "code":"adapter_reference_invalid",
            "reason":"Adapter did not return a complete policy identity", "quality_decision":"not_evaluated"});
    }
    json!({"status":"active", "source":"gitguard-policy-reference/v1",
        "reference_schema_version":1, "policy_location":"git-common-dir/gitflow.xml",
        "profile":profile, "revision":value["revision"], "policy_sha256":digest,
        "quality_decision":"not_evaluated"})
}

/// 调用已显式信任的只读适配器；返回原始策略判定，不提升为审批授权。
pub(crate) fn invoke(
    root: &std::path::Path,
    python: &std::path::Path,
    project: &std::path::Path,
    request: Option<&Value>,
) -> Result<Value, String> {
    if !root.join("scripts/gitflow/policy.py").is_file() || !python.is_file() {
        return Ok(
            json!({"status":"unverified","reason":"GitFlow adapter or Python executable missing"}),
        );
    }
    let mut child = Command::new(python)
        .args([
            "-I",
            "-B",
            "-X",
            "utf8",
            "-c",
            include_str!("gitflow_reader.py"),
        ])
        .arg(root)
        .arg(project)
        .arg(request.map(Value::to_string).unwrap_or_default())
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let output = child.stdout.take().ok_or("missing stdout")?;
    let errors = child.stderr.take().ok_or("missing stderr")?;
    let reader = thread::spawn(move || {
        let mut bytes = vec![];
        output
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let error_reader = thread::spawn(move || {
        let mut bytes = vec![];
        errors.take(65537).read_to_end(&mut bytes).map(|_| bytes)
    });
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break Some(status);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        thread::sleep(Duration::from_millis(20));
    };
    let bytes = reader
        .join()
        .map_err(|_| "adapter stdout reader failed")?
        .map_err(|e| e.to_string())?;
    let stderr = error_reader
        .join()
        .map_err(|_| "adapter stderr reader failed")?
        .map_err(|e| e.to_string())?;
    if status.is_none() || bytes.len() > 1024 * 1024 || stderr.len() > 65536 {
        return Ok(json!({"status":"unverified","reason":"adapter timeout or output limit"}));
    }
    if !status.unwrap().success() {
        return Ok(
            json!({"status":"unverified","reason":"GitFlow adapter process failed; verify adapter compatibility"}),
        );
    }
    serde_json::from_slice(&bytes).map_err(|_| "invalid GitFlow adapter JSON".into())
}
