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
    invoke(root, python, &options.project, None)
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
