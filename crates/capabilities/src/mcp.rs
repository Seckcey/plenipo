//! The Model Context Protocol (JSON-RPC 2.0) as Plenipo's tool server speaks it: `initialize`,
//! `ping`, `tools/list`, and `tools/call`. Notifications are accepted and ignored. Messages come
//! from an AI model's tool and are untrusted: anything unexpected gets a JSON-RPC error.

use serde_json::{json, Value};

use crate::broker::Broker;

/// Protocol versions this server understands, newest first.
pub const VERSIONS: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];

pub fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn result(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

/// The response to one message (`None` for a notification).
pub async fn handle(broker: &Broker, grant_id: &str, message: Value) -> Option<Value> {
    let Some(obj) = message.as_object() else {
        return Some(error(Value::Null, -32600, "Invalid request"));
    };
    let id = obj.get("id").cloned();
    let Some(method) = obj.get("method").and_then(Value::as_str) else {
        // A response or something else: nothing is ever asked of the client.
        return id.map(|id| error(id, -32600, "Invalid request"));
    };
    let id = id?; // A notification.
    if !(id.is_string() || id.is_number()) {
        return Some(error(Value::Null, -32600, "Invalid request id"));
    }
    let params = obj.get("params").cloned().unwrap_or(Value::Null);
    Some(match method {
        "initialize" => {
            let asked = params["protocolVersion"].as_str().unwrap_or("");
            let version = VERSIONS
                .iter()
                .find(|v| **v == asked)
                .copied()
                .unwrap_or(VERSIONS[0]);
            result(
                id,
                json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": { "name": "plenipo", "version": env!("CARGO_PKG_VERSION") },
                    "instructions": broker.instructions(grant_id),
                }),
            )
        }
        "ping" => result(id, json!({})),
        "tools/list" => result(id, json!({ "tools": broker.tool_list(grant_id) })),
        "tools/call" => {
            let Some(name) = params["name"].as_str() else {
                return Some(error(id, -32602, "tools/call needs a tool name"));
            };
            let args = params.get("arguments").cloned().unwrap_or(Value::Null);
            let out = broker.call(grant_id, name, args).await;
            result(
                id,
                json!({
                    "content": [{ "type": "text", "text": out.text }],
                    "isError": out.is_error,
                }),
            )
        }
        _ => error(id, -32601, "Method not found"),
    })
}
