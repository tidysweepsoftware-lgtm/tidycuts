//! The Model Context Protocol part of the server: JSON-RPC 2.0 messages in,
//! JSON-RPC messages out. Knows nothing about sockets or about TidyCuts; what
//! the tools do is the [`Host`]'s business.

use serde_json::{json, Value};
use std::panic::{catch_unwind, AssertUnwindSafe};

/// Protocol versions we can speak, newest first.
pub const SUPPORTED_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

/// One tool Claude can call.
#[derive(Debug, Clone)]
pub struct Tool {
    pub name: String,
    pub description: String,
    /// JSON Schema of the arguments (an object schema).
    pub schema: Value,
    /// The tool only looks; it never changes the project.
    pub read_only: bool,
}

/// One piece of a tool's answer.
#[derive(Debug, Clone, PartialEq)]
pub enum Content {
    Text(String),
    /// A picture Claude can look at.
    Image {
        data: Vec<u8>,
        mime: String,
    },
}

/// What a tool call gives back.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolResult {
    pub content: Vec<Content>,
    pub is_error: bool,
}

impl ToolResult {
    pub fn text(text: impl Into<String>) -> ToolResult {
        ToolResult {
            content: vec![Content::Text(text.into())],
            is_error: false,
        }
    }

    /// A failure Claude can read and react to (the call itself worked).
    pub fn error(text: impl Into<String>) -> ToolResult {
        ToolResult {
            content: vec![Content::Text(text.into())],
            is_error: true,
        }
    }

    /// A JSON value as pretty text.
    pub fn json(value: &Value) -> ToolResult {
        ToolResult::text(serde_json::to_string_pretty(value).unwrap_or_default())
    }

    pub fn with_image(mut self, data: Vec<u8>, mime: &str) -> ToolResult {
        self.content.push(Content::Image {
            data,
            mime: mime.into(),
        });
        self
    }

    fn to_json(&self) -> Value {
        let content: Vec<Value> = self
            .content
            .iter()
            .map(|c| match c {
                Content::Text(t) => json!({"type": "text", "text": t}),
                Content::Image { data, mime } => {
                    json!({"type": "image", "data": base64(data), "mimeType": mime})
                }
            })
            .collect();
        json!({"content": content, "isError": self.is_error})
    }
}

/// What the server runs the tools on.
pub trait Host: Send + Sync {
    /// Shown to Claude when it connects: what this server is and how to use it.
    fn instructions(&self) -> String;
    fn tools(&self) -> Vec<Tool>;
    /// Runs a tool. May block (the server calls it on a connection thread).
    fn call(&self, name: &str, args: &Value) -> ToolResult;
}

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

fn error(id: Value, code: i64, message: impl Into<String>) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message.into()}})
}

fn ok(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

/// What to send back for a request body.
#[derive(Debug, PartialEq)]
pub enum Reply {
    /// A JSON-RPC response (or batch of them).
    Json(Value),
    /// Only notifications or responses came in: nothing to say.
    Accepted,
}

/// Handles one HTTP body: a message or a batch of them.
pub fn handle_body(host: &dyn Host, body: &[u8]) -> Reply {
    let Ok(value) = serde_json::from_slice::<Value>(body) else {
        return Reply::Json(error(Value::Null, PARSE_ERROR, "That is not valid JSON."));
    };
    match value {
        Value::Array(items) if items.is_empty() => Reply::Json(error(
            Value::Null,
            INVALID_REQUEST,
            "An empty batch is not allowed.",
        )),
        Value::Array(items) => {
            let replies: Vec<Value> = items.iter().filter_map(|m| handle(host, m)).collect();
            if replies.is_empty() {
                Reply::Accepted
            } else {
                Reply::Json(Value::Array(replies))
            }
        }
        other => match handle(host, &other) {
            Some(r) => Reply::Json(r),
            None => Reply::Accepted,
        },
    }
}

/// Handles one message; `None` for notifications and client responses.
pub fn handle(host: &dyn Host, msg: &Value) -> Option<Value> {
    let Some(obj) = msg.as_object() else {
        return Some(error(
            Value::Null,
            INVALID_REQUEST,
            "A message must be a JSON object.",
        ));
    };
    let id = obj.get("id").cloned();
    let Some(method) = obj.get("method").and_then(Value::as_str) else {
        // A response to something we asked (we never ask): ignore it.
        if obj.contains_key("result") || obj.contains_key("error") {
            return None;
        }
        return Some(error(
            id.unwrap_or(Value::Null),
            INVALID_REQUEST,
            "The message has no method.",
        ));
    };
    // Notifications have no id and get no answer.
    let id = id?;
    let params = obj.get("params").cloned().unwrap_or(Value::Null);
    Some(match method {
        "initialize" => ok(id, initialize(host, &params)),
        "ping" => ok(id, json!({})),
        "tools/list" => ok(id, tools_list(host)),
        "tools/call" => match tools_call(host, &params) {
            Ok(r) => ok(id, r),
            Err((code, msg)) => error(id, code, msg),
        },
        other => error(id, METHOD_NOT_FOUND, format!("Unknown method: {other}")),
    })
}

fn initialize(host: &dyn Host, params: &Value) -> Value {
    let wanted = params.get("protocolVersion").and_then(Value::as_str);
    let version = wanted
        .filter(|w| SUPPORTED_VERSIONS.contains(w))
        .unwrap_or(SUPPORTED_VERSIONS[0]);
    json!({
        "protocolVersion": version,
        "capabilities": {"tools": {"listChanged": false}},
        "serverInfo": {"name": "tidycuts", "version": env!("CARGO_PKG_VERSION")},
        "instructions": host.instructions(),
    })
}

fn tools_list(host: &dyn Host) -> Value {
    let tools: Vec<Value> = host
        .tools()
        .into_iter()
        .map(|t| {
            json!({
                "name": t.name,
                "description": t.description,
                "inputSchema": t.schema,
                "annotations": {"readOnlyHint": t.read_only},
            })
        })
        .collect();
    json!({"tools": tools})
}

fn tools_call(host: &dyn Host, params: &Value) -> Result<Value, (i64, String)> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or((INVALID_PARAMS, "tools/call needs a tool name.".to_string()))?;
    if !host.tools().iter().any(|t| t.name == name) {
        return Err((INVALID_PARAMS, format!("Unknown tool: {name}")));
    }
    let args = match params.get("arguments") {
        None | Some(Value::Null) => json!({}),
        Some(a @ Value::Object(_)) => a.clone(),
        Some(_) => {
            return Err((
                INVALID_PARAMS,
                "A tool's arguments must be a JSON object.".into(),
            ))
        }
    };
    // A bug in one tool must not take the server down.
    let result = catch_unwind(AssertUnwindSafe(|| host.call(name, &args)))
        .unwrap_or_else(|_| ToolResult::error(format!("The {name} tool crashed.")));
    Ok(result.to_json())
}

/// Standard base64 with padding.
pub fn base64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (chunk[0] as u32) << 16
            | (*chunk.get(1).unwrap_or(&0) as u32) << 8
            | *chunk.get(2).unwrap_or(&0) as u32;
        out.push(A[(n >> 18) as usize & 63] as char);
        out.push(A[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            A[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            A[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake;

    impl Host for Fake {
        fn instructions(&self) -> String {
            "Edit videos.".into()
        }
        fn tools(&self) -> Vec<Tool> {
            let object = json!({"type": "object", "properties": {}});
            vec![
                Tool {
                    name: "echo".into(),
                    description: "Says back what it is told.".into(),
                    schema: object.clone(),
                    read_only: true,
                },
                Tool {
                    name: "boom".into(),
                    description: "Always crashes.".into(),
                    schema: object,
                    read_only: false,
                },
            ]
        }
        fn call(&self, name: &str, args: &Value) -> ToolResult {
            match name {
                "echo" => ToolResult::json(args).with_image(vec![1, 2, 3], "image/png"),
                _ => panic!("boom"),
            }
        }
    }

    fn call(msg: Value) -> Option<Value> {
        handle(&Fake, &msg)
    }

    #[test]
    fn base64_matches_the_standard_examples() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn initialize_picks_a_version_both_sides_know() {
        let r = call(json!({"jsonrpc":"2.0","id":1,"method":"initialize",
            "params":{"protocolVersion":"2025-03-26"}}))
        .unwrap();
        assert_eq!(r["id"], 1);
        assert_eq!(r["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(r["result"]["serverInfo"]["name"], "tidycuts");
        assert_eq!(r["result"]["instructions"], "Edit videos.");
        assert!(r["result"]["capabilities"]["tools"].is_object());
        // An unknown future version gets our newest.
        let r = call(json!({"jsonrpc":"2.0","id":2,"method":"initialize",
            "params":{"protocolVersion":"2099-01-01"}}))
        .unwrap();
        assert_eq!(r["result"]["protocolVersion"], SUPPORTED_VERSIONS[0]);
    }

    #[test]
    fn notifications_and_client_responses_get_no_answer() {
        assert!(call(json!({"jsonrpc":"2.0","method":"notifications/initialized"})).is_none());
        assert!(call(json!({"jsonrpc":"2.0","id":7,"result":{}})).is_none());
    }

    #[test]
    fn tools_are_listed_with_schemas_and_hints() {
        let r = call(json!({"jsonrpc":"2.0","id":3,"method":"tools/list"})).unwrap();
        let tools = r["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0]["name"], "echo");
        assert_eq!(tools[0]["inputSchema"]["type"], "object");
        assert_eq!(tools[0]["annotations"]["readOnlyHint"], true);
        assert_eq!(tools[1]["annotations"]["readOnlyHint"], false);
    }

    #[test]
    fn calls_return_text_and_images() {
        let r = call(json!({"jsonrpc":"2.0","id":4,"method":"tools/call",
            "params":{"name":"echo","arguments":{"a":1}}}))
        .unwrap();
        let c = &r["result"]["content"];
        assert_eq!(c[0]["type"], "text");
        assert!(c[0]["text"].as_str().unwrap().contains("\"a\": 1"));
        assert_eq!(c[1]["type"], "image");
        assert_eq!(c[1]["data"], "AQID");
        assert_eq!(c[1]["mimeType"], "image/png");
        assert_eq!(r["result"]["isError"], false);
    }

    #[test]
    fn bad_calls_get_protocol_errors_and_crashes_are_contained() {
        let r = call(json!({"jsonrpc":"2.0","id":5,"method":"tools/call",
            "params":{"name":"nope"}}))
        .unwrap();
        assert_eq!(r["error"]["code"], INVALID_PARAMS);
        let r = call(json!({"jsonrpc":"2.0","id":6,"method":"tools/call",
            "params":{"name":"echo","arguments":[1]}}))
        .unwrap();
        assert_eq!(r["error"]["code"], INVALID_PARAMS);
        let r = call(json!({"jsonrpc":"2.0","id":8,"method":"tools/call",
            "params":{"name":"boom"}}))
        .unwrap();
        assert_eq!(r["result"]["isError"], true);
        let r = call(json!({"jsonrpc":"2.0","id":9,"method":"frobnicate"})).unwrap();
        assert_eq!(r["error"]["code"], METHOD_NOT_FOUND);
        assert_eq!(call(json!([1])).unwrap()["error"]["code"], INVALID_REQUEST);
    }

    #[test]
    fn bodies_may_be_batches_or_garbage() {
        let batch = br#"[{"jsonrpc":"2.0","id":1,"method":"ping"},
            {"jsonrpc":"2.0","method":"notifications/initialized"}]"#;
        let Reply::Json(Value::Array(r)) = handle_body(&Fake, batch) else {
            panic!("batch reply")
        };
        assert_eq!(r.len(), 1);
        assert_eq!(
            handle_body(&Fake, br#"{"jsonrpc":"2.0","method":"notifications/x"}"#),
            Reply::Accepted
        );
        let Reply::Json(e) = handle_body(&Fake, b"{nope") else {
            panic!("error reply")
        };
        assert_eq!(e["error"]["code"], PARSE_ERROR);
        let Reply::Json(e) = handle_body(&Fake, b"[]") else {
            panic!("error reply")
        };
        assert_eq!(e["error"]["code"], INVALID_REQUEST);
    }
}
