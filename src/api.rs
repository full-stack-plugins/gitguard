//! Versioned local read-only port. No credential provider or mutation dispatcher.
use crate::{cli::CheckRequest, evidence::envelope::BoundCheck};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{io::Read, sync::atomic::AtomicBool};
pub const VERSION: &str = "gitguard.api/v1alpha1";
pub const MAX_INPUT: usize = 1_048_576;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    version: String,
    capability: String,
    request: Option<CheckRequest>,
}
pub fn capabilities() -> Value {
    json!({"version":VERSION,"read":["check"],"write":false,"credentials":"unavailable","authorization":"not-evaluated","profile":"local-advisory"})
}
pub fn error(code: &str) -> Value {
    json!({"version":VERSION,"exitCode":4,"diagnostic":code,"bundle":null})
}
/// Bounded byte entry point shared by both new transports. Core validates frozen binding/policy.
pub fn dispatch(bytes: &[u8], cancelled: &AtomicBool) -> Value {
    if bytes.len() > MAX_INPUT {
        return error("input_limit");
    }
    let request: Request = match serde_json::from_slice(bytes) {
        Ok(value) => value,
        Err(_) => return error("invalid_request"),
    };
    if request.version != VERSION {
        return error("unsupported_version");
    }
    match (request.capability.as_str(), request.request) {
        ("capabilities", None) => {
            json!({"version":VERSION,"exitCode":0,"capabilities":capabilities()})
        }
        ("check", Some(request)) => match BoundCheck::prepare(request) {
            Err(_) => error("preflight_failed"),
            Ok(bound) => match bound.run(cancelled) {
                Ok(bundle) => {
                    json!({"version":VERSION,"exitCode":bundle.exit_code(),"bundle":bundle})
                }
                Err(_) => error("bound_check_failed"),
            },
        },
        ("check" | "capabilities", _) => error("invalid_request"),
        _ => error("unsupported_capability"),
    }
}
pub fn read_bounded(reader: impl Read) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take((MAX_INPUT + 1) as u64)
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

// Reject ambiguous duplicate members before the MCP envelope becomes a Value.
// serde_json's default recursion limit remains enabled and byte size is checked first.
pub(crate) fn unique_json(bytes: &[u8]) -> Result<Value, serde_json::Error> {
    struct Unique(Value);
    impl<'de> Deserialize<'de> for Unique {
        fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            struct Visitor;
            impl<'de> serde::de::Visitor<'de> for Visitor {
                type Value = Unique;
                fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                    f.write_str("unambiguous JSON")
                }
                fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Unique, E> {
                    Ok(Unique(v.into()))
                }
                fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Unique, E> {
                    Ok(Unique(v.into()))
                }
                fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Unique, E> {
                    Ok(Unique(v.into()))
                }
                fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Unique, E> {
                    Ok(Unique(json!(v)))
                }
                fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Unique, E> {
                    Ok(Unique(v.into()))
                }
                fn visit_unit<E: serde::de::Error>(self) -> Result<Unique, E> {
                    Ok(Unique(Value::Null))
                }
                fn visit_seq<A: serde::de::SeqAccess<'de>>(
                    self,
                    mut a: A,
                ) -> Result<Unique, A::Error> {
                    let mut values = vec![];
                    while let Some(Unique(v)) = a.next_element()? {
                        values.push(v);
                    }
                    Ok(Unique(Value::Array(values)))
                }
                fn visit_map<A: serde::de::MapAccess<'de>>(
                    self,
                    mut a: A,
                ) -> Result<Unique, A::Error> {
                    let mut values = serde_json::Map::new();
                    while let Some(key) = a.next_key::<String>()? {
                        if values.contains_key(&key) {
                            return Err(serde::de::Error::custom("duplicate member"));
                        }
                        let Unique(v) = a.next_value()?;
                        values.insert(key, v);
                    }
                    Ok(Unique(Value::Object(values)))
                }
            }
            d.deserialize_any(Visitor)
        }
    }
    serde_json::from_slice::<Unique>(bytes).map(|v| v.0)
}
