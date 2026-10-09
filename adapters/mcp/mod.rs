//! Conservative MCP 2025-11-25 local stdio profile. No network or write tools.
use crate::api;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    io::{BufRead, Read, Write},
    sync::atomic::AtomicBool,
};
pub const PROTOCOL_VERSION: &str = "2025-11-25";
#[derive(Default)]
pub struct Session {
    initialized: bool,
    ready: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Message {
    jsonrpc: String,
    #[serde(default, deserialize_with = "present_id")]
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Initialize {
    protocol_version: String,
    capabilities: Value,
    client_info: ClientInfo,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ClientInfo {
    name: String,
    version: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Call {
    name: String,
    arguments: Value,
}
fn present_id<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(d).map(Some)
}
fn failure(id: Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}
fn response(id: Value, result: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"result":result})
}
fn empty(params: &Option<Value>) -> bool {
    params
        .as_ref()
        .is_none_or(|p| p.as_object().is_some_and(|o| o.is_empty()))
}
impl Session {
    /// One bounded JSON-RPC message; successful notifications produce no reply.
    pub fn handle(&mut self, bytes: &[u8], cancel: &AtomicBool) -> Option<Value> {
        if bytes.len() > api::MAX_INPUT {
            return Some(failure(Value::Null, -32600, "input limit"));
        }
        let message: Message = match api::unique_json(bytes).and_then(serde_json::from_value) {
            Ok(v) => v,
            Err(_) => return Some(failure(Value::Null, -32700, "invalid message")),
        };
        let id = message.id.clone().unwrap_or(Value::Null);
        if message.jsonrpc != "2.0"
            || message
                .id
                .as_ref()
                .is_some_and(|id| !id.is_string() && !id.is_i64() && !id.is_u64())
        {
            return Some(failure(Value::Null, -32600, "invalid request"));
        }
        if message.id.is_none() {
            if message.method == "notifications/initialized"
                && self.initialized
                && empty(&message.params)
            {
                self.ready = true;
            }
            return None;
        }
        if message.method == "ping" && empty(&message.params) {
            return Some(response(id, json!({})));
        }
        if message.method == "initialize" {
            if self.initialized {
                return Some(failure(id, -32600, "already initialized"));
            }
            let init: Initialize =
                match serde_json::from_value(message.params.unwrap_or(Value::Null)) {
                    Ok(v) => v,
                    Err(_) => return Some(failure(id, -32602, "invalid initialize")),
                };
            if init.protocol_version != PROTOCOL_VERSION
                || init.capabilities != json!({})
                || init.client_info.name.is_empty()
                || init.client_info.version.is_empty()
            {
                return Some(failure(
                    id,
                    -32602,
                    "unsupported version or capability profile",
                ));
            }
            self.initialized = true;
            return Some(response(
                id,
                json!({"protocolVersion":PROTOCOL_VERSION,"capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":"gitguard","version":env!("CARGO_PKG_VERSION")},"instructions":"Local advisory observation only. Writes and credentials unavailable. ALLOW is not authorization."}),
            ));
        }
        if !self.ready {
            return Some(failure(id, -32600, "initialize first"));
        }
        match message.method.as_str() {
            "tools/list" if empty(&message.params) => Some(response(
                id,
                json!({"tools":[{"name":"gitguard_check","description":"Read-only frozen candidate check; no write authorization or credentials","inputSchema":{"type":"object","additionalProperties":false,"required":["version","capability","request"],"properties":{"version":{"const":api::VERSION},"capability":{"const":"check"},"request":{"type":"object"}}},"annotations":{"readOnlyHint":true,"destructiveHint":false,"idempotentHint":true,"openWorldHint":false},"execution":{"taskSupport":"forbidden"}}]}),
            )),
            "tools/call" => {
                let call: Call = match serde_json::from_value(message.params.unwrap_or(Value::Null))
                {
                    Ok(v) => v,
                    Err(_) => return Some(failure(id, -32602, "invalid tool arguments")),
                };
                if call.name != "gitguard_check" || call.arguments["capability"] != "check" {
                    return Some(failure(id, -32602, "unsupported tool or capability"));
                }
                let result = api::dispatch(
                    &serde_json::to_vec(&call.arguments).expect("JSON value"),
                    cancel,
                );
                let text = serde_json::to_string(&result).expect("JSON value");
                Some(response(
                    id,
                    json!({"isError":result["exitCode"]==4,"content":[{"type":"text","text":text}],"structuredContent":result}),
                ))
            }
            "tools/list" => Some(failure(id, -32602, "unsupported list parameters")),
            _ => Some(failure(id, -32601, "unsupported method")),
        }
    }
}
/// Newline-framed process-local transport; never binds sockets or configures a host.
pub fn serve(
    mut input: impl BufRead,
    mut output: impl Write,
    cancel: &AtomicBool,
) -> std::io::Result<()> {
    let mut session = Session::default();
    loop {
        let mut line = Vec::new();
        let count = input
            .by_ref()
            .take((api::MAX_INPUT + 2) as u64)
            .read_until(b'\n', &mut line)?;
        if count == 0 {
            return Ok(());
        }
        if line.last() == Some(&b'\n') {
            line.pop();
        }
        if line.len() > api::MAX_INPUT {
            serde_json::to_writer(&mut output, &failure(Value::Null, -32600, "input limit"))?;
            output.write_all(b"\n")?;
            output.flush()?;
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "input limit",
            ));
        }
        if let Some(reply) = session.handle(&line, cancel) {
            serde_json::to_writer(&mut output, &reply)?;
            output.write_all(b"\n")?;
            output.flush()?;
        }
    }
}
