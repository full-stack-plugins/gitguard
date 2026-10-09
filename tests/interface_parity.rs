mod common;
use common::*;
use gitguard::{
    candidate::CandidateRequest, cli::CheckRequest, evidence::projection::FrozenPolicy,
    scope::TaskScope,
};
use guardengine::Enforcement;
use serde_json::{Value, json};
use std::{
    io::Write,
    process::{Command, Stdio},
};
fn request(r: &Repo, oid: &str, enforcement: Enforcement, path: &str) -> Value {
    let policy = FrozenPolicy::new(enforcement);
    serde_json::to_value(CheckRequest {
        api_version: "gitguard.check/v1alpha1".into(),
        repo_root: r.path().into(),
        repo_id: "repo".into(),
        candidate_oid: oid.into(),
        scope: TaskScope::advisory(
            "task",
            vec!["R".into()],
            vec![path.as_bytes().to_vec()],
            &policy.digest(),
            None,
        )
        .unwrap(),
        candidate: CandidateRequest {
            worktree_id: "w".into(),
            base_oid: r.base.clone(),
            merge_group_id: None,
            members: vec![],
        },
        contract: policy.contract(),
    })
    .unwrap()
}
fn call(command: &str, bytes: &[u8]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_gitguard"))
        .arg(command)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    child.wait_with_output().unwrap()
}
fn api(req: Value) -> Value {
    json!({"version":"gitguard.api/v1alpha1","capability":"check","request":req})
}
fn semantic(v: &Value) -> Value {
    json!({"binding":v["envelope"]["binding"],"coverage":v["envelope"]["coverage"],"status":v["envelope"]["runStatus"],"decision":v["envelope"]["decision"],"diagnostics":v["envelope"]["diagnostics"],"domain":v["domain"],"facts":v["facts"],"report":v["report"],"contract":v["contract"]})
}
#[test]
fn versioned_api_preserves_real_cli_candidate_scope_and_outcome() {
    let r = Repo::new("sha1");
    let head = r.commit("a", "candidate");
    let before = r.state();
    for (mode, path, code) in [
        (Enforcement::Enforce, "a", 0),
        (Enforcement::Enforce, "other", 2),
        (Enforcement::Review, "other", 3),
    ] {
        let req = request(&r, &head, mode, path);
        let old = call("check", &serde_json::to_vec(&req).unwrap());
        assert_eq!(old.status.code(), Some(code));
        let out = call("api", &serde_json::to_vec(&api(req)).unwrap());
        assert_eq!(
            out.status.code(),
            Some(code),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let result: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(result["exitCode"], code);
        assert_eq!(
            semantic(&result["bundle"]),
            semantic(&serde_json::from_slice(&old.stdout).unwrap())
        );
    }
    assert_eq!(before, r.state());
}
#[test]
fn mcp_negotiates_read_only_without_a_repository() {
    let input = concat!(
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-11-25\",\"capabilities\":{},\"clientInfo\":{\"name\":\"fixture\",\"version\":\"1\"}}}\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}\n"
    );
    let out = call("mcp-stdio", input.as_bytes());
    assert!(out.status.success());
    let lines: Vec<Value> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["result"]["protocolVersion"], "2025-11-25");
    assert_eq!(
        lines[1]["result"]["tools"][0]["annotations"]["readOnlyHint"],
        true
    );
    assert_eq!(lines[1]["result"]["tools"].as_array().unwrap().len(), 1);
}
fn initialize(session: &mut gitguard::mcp::Session) {
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let init = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"}}});
    assert!(
        session
            .handle(&serde_json::to_vec(&init).unwrap(), &cancel)
            .unwrap()
            .get("result")
            .is_some()
    );
    assert!(
        session
            .handle(
                br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
                &cancel
            )
            .is_none()
    );
}
#[test]
fn all_three_ports_share_core_bindings_diagnostics_and_cancelled_null() {
    use std::sync::atomic::AtomicBool;
    let r = Repo::new("sha1");
    let head = r.commit("a", "candidate");
    for dirty in [false, true] {
        if dirty {
            std::fs::write(r.path().join("a"), "dirty").unwrap();
        }
        let before = r.state();
        for (mode, path) in [
            (Enforcement::Enforce, "a"),
            (Enforcement::Enforce, "other"),
            (Enforcement::Review, "other"),
        ] {
            let req = request(&r, &head, mode, path);
            let old = call("check", &serde_json::to_vec(&req).unwrap());
            let plain: Value = serde_json::from_slice(&old.stdout).unwrap();
            let args = api(req);
            let direct = gitguard::api::dispatch(
                &serde_json::to_vec(&args).unwrap(),
                &AtomicBool::new(false),
            );
            let mut session = gitguard::mcp::Session::default();
            initialize(&mut session);
            let rpc = json!({"jsonrpc":"2.0","id":"read-1","method":"tools/call","params":{"name":"gitguard_check","arguments":args}});
            let result = session
                .handle(&serde_json::to_vec(&rpc).unwrap(), &AtomicBool::new(false))
                .unwrap();
            let init = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"}}});
            let frames = format!(
                "{}\n{}\n{}\n",
                init,
                json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                rpc
            );
            let executable = call("mcp-stdio", frames.as_bytes());
            assert!(executable.status.success());
            let replies: Vec<Value> = String::from_utf8(executable.stdout)
                .unwrap()
                .lines()
                .map(|line| serde_json::from_str(line).unwrap())
                .collect();
            assert_eq!(replies.len(), 2);
            assert_eq!(
                semantic(&plain),
                semantic(&replies[1]["result"]["structuredContent"]["bundle"])
            );
            let mcp = &result["result"]["structuredContent"];
            assert_eq!(semantic(&plain), semantic(&direct["bundle"]));
            assert_eq!(semantic(&plain), semantic(&mcp["bundle"]));
            assert_eq!(mcp["exitCode"], old.status.code().unwrap());
            assert_eq!(
                serde_json::from_str::<Value>(
                    result["result"]["content"][0]["text"].as_str().unwrap()
                )
                .unwrap(),
                *mcp
            );
            let cancelled = gitguard::api::dispatch(
                &serde_json::to_vec(&args).unwrap(),
                &AtomicBool::new(true),
            );
            assert_eq!(cancelled["exitCode"], 4);
            assert!(cancelled["bundle"]["envelope"]["decision"].is_null());
            assert_eq!(cancelled["bundle"]["envelope"]["runStatus"], "cancelled");
            let rpc_cancelled = session
                .handle(&serde_json::to_vec(&rpc).unwrap(), &AtomicBool::new(true))
                .unwrap();
            assert_eq!(
                semantic(&cancelled["bundle"]),
                semantic(&rpc_cancelled["result"]["structuredContent"]["bundle"])
            );
        }
        assert_eq!(before, r.state());
    }
}
#[test]
fn unknown_versions_writes_credentials_and_bad_bindings_fail_without_results() {
    use std::sync::atomic::AtomicBool;
    let r = Repo::new("sha1");
    let before = r.state();
    let req = request(&r, &r.base, Enforcement::Enforce, "a");
    let cancel = AtomicBool::new(false);
    for cap in ["push", "merge", "apply", "write", "check/v2"] {
        let value = json!({"version":"gitguard.api/v1alpha1","capability":cap});
        let result = gitguard::api::dispatch(&serde_json::to_vec(&value).unwrap(), &cancel);
        assert_eq!(result["diagnostic"], "unsupported_capability");
        assert!(result["bundle"].is_null());
    }
    for mut value in [
        api(req.clone()),
        api(req.clone()),
        api(req.clone()),
        api(req.clone()),
    ]
    .into_iter()
    .enumerate()
    {
        match value.0 {
            0 => value.1["version"] = json!("future"),
            1 => value.1["credentials"] = json!("secret"),
            2 => value.1["request"]["candidate_oid"] = json!("0".repeat(40)),
            _ => value.1["request"]["scope"]["policy_digest"] = json!("wrong"),
        }
        let result = gitguard::api::dispatch(&serde_json::to_vec(&value.1).unwrap(), &cancel);
        assert_eq!(result["exitCode"], 4);
        assert!(result["bundle"].is_null());
        assert!(!result.to_string().contains("secret"));
    }
    let caps = gitguard::api::dispatch(
        br#"{"version":"gitguard.api/v1alpha1","capability":"capabilities"}"#,
        &cancel,
    );
    assert_eq!(caps["capabilities"]["write"], false);
    assert_eq!(caps["capabilities"]["credentials"], "unavailable");
    assert_eq!(before, r.state());
}
#[test]
fn mcp_lifecycle_unknown_tools_and_versions_are_rejected() {
    use std::sync::atomic::AtomicBool;
    let c = AtomicBool::new(false);
    let mut session = gitguard::mcp::Session::default();
    assert!(
        session
            .handle(br#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#, &c)
            .unwrap()
            .get("error")
            .is_some()
    );
    for (version, capabilities) in [("future", json!({})), ("2025-11-25", json!({"write":true}))] {
        let init = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":version,"capabilities":capabilities,"clientInfo":{"name":"test","version":"1"}}});
        assert!(
            session
                .handle(&serde_json::to_vec(&init).unwrap(), &c)
                .unwrap()
                .get("error")
                .is_some()
        );
    }
    initialize(&mut session);
    for msg in [
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"push","arguments":{}}}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"gitguard_check","arguments":{"capability":"write"}}}),
        json!({"jsonrpc":"2.0","id":2,"method":"resources/list"}),
    ] {
        assert!(
            session
                .handle(&serde_json::to_vec(&msg).unwrap(), &c)
                .unwrap()
                .get("error")
                .is_some()
        );
    }
}
#[test]
fn bounded_frames_reject_oversize_without_consuming_a_followup_request() {
    use std::sync::atomic::AtomicBool;
    let oversized = vec![b' '; gitguard::api::MAX_INPUT + 2];
    let c = AtomicBool::new(false);
    assert_eq!(
        gitguard::api::dispatch(&oversized, &c)["diagnostic"],
        "input_limit"
    );
    let mut output = vec![];
    assert!(gitguard::mcp::serve(std::io::Cursor::new(&oversized), &mut output, &c).is_err());
    assert_eq!(String::from_utf8(output).unwrap().lines().count(), 1);
}
#[test]
fn mcp_null_id_and_duplicate_capability_do_not_execute() {
    use std::sync::atomic::AtomicBool;
    let c = AtomicBool::new(false);
    let mut session = gitguard::mcp::Session::default();
    initialize(&mut session);
    assert!(
        session
            .handle(br#"{"jsonrpc":"2.0","id":null,"method":"tools/list"}"#, &c)
            .unwrap()
            .get("error")
            .is_some()
    );
    let duplicated=br#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"gitguard_check","arguments":{"version":"gitguard.api/v1alpha1","capability":"write","capability":"check"}}}"#;
    assert!(
        session
            .handle(duplicated, &c)
            .unwrap()
            .get("error")
            .is_some()
    );
}
#[test]
fn binary_error_transport_is_explicit_and_does_not_expose_input() {
    let malformed = call("api", br#"{"password":"never-echo-this"}"#);
    assert_eq!(malformed.status.code(), Some(4));
    let value: Value = serde_json::from_slice(&malformed.stdout).unwrap();
    assert_eq!(value["diagnostic"], "invalid_request");
    assert!(
        !String::from_utf8(malformed.stdout)
            .unwrap()
            .contains("never-echo-this")
    );
    let mut session = gitguard::mcp::Session::default();
    initialize(&mut session);
    let c = std::sync::atomic::AtomicBool::new(false);
    let reply=session.handle(br#"{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"gitguard_check","arguments":{"version":"gitguard.api/v1alpha1","capability":"check","request":{}}}}"#,&c).unwrap();
    assert_eq!(reply["result"]["isError"], true);
    assert!(reply["result"]["structuredContent"]["bundle"].is_null());
}
