use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::net::TcpStream;

const DEFAULT_AGENT_API: &str = "127.0.0.1:8080";

fn main() {
    let agent_api = std::env::var("AGENT_API_ADDR").unwrap_or_else(|_| DEFAULT_AGENT_API.into());
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(line) if !line.trim().is_empty() => line,
            Ok(_) => continue,
            Err(error) => {
                eprintln!("MCP stdin error: {error}");
                break;
            }
        };

        let request: Value = match serde_json::from_str(&line) {
            Ok(request) => request,
            Err(error) => {
                write_response(
                    &mut stdout,
                    json_rpc_error(Value::Null, -32700, format!("parse error: {error}")),
                );
                continue;
            }
        };

        if request.get("method").and_then(Value::as_str) == Some("notifications/initialized") {
            continue;
        }

        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let response = match dispatch(&agent_api, &request) {
            Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
            Err(error) => json_rpc_error(id, error_code(&error), error),
        };
        write_response(&mut stdout, response);
    }
}

fn dispatch(agent_api: &str, request: &Value) -> Result<Value, String> {
    match request.get("method").and_then(Value::as_str) {
        Some("initialize") => Ok(json!({
            "protocolVersion": "2025-06-18",
            "capabilities": {
                "tools": {},
                "resources": {}
            },
            "serverInfo": {
                "name": "slint-agent-desktop",
                "version": env!("CARGO_PKG_VERSION")
            }
        })),
        Some("tools/list") => Ok(tool_definitions()),
        Some("resources/list") => Ok(json!({
            "resources": [
                {"uri": "app://state", "name": "Application state", "mimeType": "application/json"},
                {"uri": "ui://tree", "name": "Semantic UI tree", "mimeType": "application/json"},
                {"uri": "app://logs", "name": "Recent application logs", "mimeType": "text/plain"}
            ]
        })),
        Some("resources/read") => {
            let uri = required_string(request, "/params/uri")?;
            read_resource(agent_api, &uri)
        }
        Some("tools/call") => {
            let name = required_string(request, "/params/name")?;
            let arguments = request
                .pointer("/params/arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            call_tool(agent_api, &name, &arguments)
        }
        Some("ping") => Ok(json!({})),
        Some(method) => Err(format!("method not found: {method}")),
        None => Err("missing JSON-RPC method".into()),
    }
}

fn tool_definitions() -> Value {
    json!({
        "tools": [
            {
                "name": "app_state",
                "description": "Inspect authoritative application state.",
                "inputSchema": {"type": "object", "properties": {}, "additionalProperties": false}
            },
            {
                "name": "ui_inspect",
                "description": "Inspect the stable semantic UI tree before acting.",
                "inputSchema": {"type": "object", "properties": {}, "additionalProperties": false}
            },
            {
                "name": "app_command",
                "description": "Execute an application intent through the application layer.",
                "inputSchema": {
                    "type": "object",
                    "required": ["command"],
                    "properties": {
                        "command": {"type": "string"},
                        "value": {"type": "string"}
                    },
                    "additionalProperties": false
                }
            },
            {
                "name": "ui_action",
                "description": "Perform a semantic UI action by stable element ID.",
                "inputSchema": {
                    "type": "object",
                    "required": ["action", "id"],
                    "properties": {
                        "action": {"type": "string", "enum": ["click", "set_value", "get_value", "focus"]},
                        "id": {"type": "string"},
                        "value": {"type": "string"}
                    },
                    "additionalProperties": false
                }
            },
            {
                "name": "ui_screenshot",
                "description": "Capture the current virtual display for visual verification.",
                "inputSchema": {
                    "type": "object",
                    "properties": {"name": {"type": "string"}},
                    "additionalProperties": false
                }
            }
        ]
    })
}

fn call_tool(agent_api: &str, name: &str, arguments: &Value) -> Result<Value, String> {
    let (text, is_error) = match name {
        "app_state" => (http_get(agent_api, "/agent/state")?, false),
        "ui_inspect" => (http_get(agent_api, "/agent/ui")?, false),
        "app_command" => {
            let command = required_string_value(arguments, "command")?;
            let value = arguments.get("value").and_then(Value::as_str).unwrap_or("");
            (
                http_post(
                    agent_api,
                    "/agent/command",
                    &json!({"command": command, "arguments": {"value": value}}),
                )?,
                false,
            )
        }
        "ui_action" => {
            let action = required_string_value(arguments, "action")?;
            let id = required_string_value(arguments, "id")?;
            let body = json!({
                "action": action,
                "id": id,
                "value": arguments.get("value").and_then(Value::as_str)
            });
            (http_post(agent_api, "/agent/ui/action", &body)?, false)
        }
        "ui_screenshot" => {
            let name = arguments
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("mcp");
            if !name.chars().all(|character| {
                character.is_ascii_alphanumeric() || character == '-' || character == '_'
            }) {
                return Err(
                    "screenshot name may contain only ASCII letters, numbers, '-' or '_'".into(),
                );
            }
            let path = format!("artifacts/screenshots/{name}.png");
            let status = std::process::Command::new("import")
                .args(["-window", "root", &path])
                .env(
                    "DISPLAY",
                    std::env::var("DISPLAY").unwrap_or_else(|_| ":1".into()),
                )
                .status()
                .map_err(|error| format!("screenshot command failed: {error}"))?;
            if !status.success() {
                return Err(format!("screenshot command exited with {status}"));
            }
            (json!({"path": path}).to_string(), false)
        }
        _ => return Err(format!("unknown tool: {name}")),
    };

    let structured = serde_json::from_str::<Value>(&text).ok();
    let mut result = json!({
        "content": [{"type": "text", "text": text}],
        "isError": is_error
    });
    if let Some(structured) = structured {
        result["structuredContent"] = structured;
    }
    Ok(result)
}

fn read_resource(agent_api: &str, uri: &str) -> Result<Value, String> {
    let (mime_type, text) = match uri {
        "app://state" => ("application/json", http_get(agent_api, "/agent/state")?),
        "ui://tree" => ("application/json", http_get(agent_api, "/agent/ui")?),
        "app://logs" => (
            "text/plain",
            std::process::Command::new("/workspace/scripts/agent")
                .arg("logs")
                .output()
                .map_err(|error| format!("log command failed: {error}"))
                .and_then(|output| {
                    if output.status.success() {
                        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
                    } else {
                        Err(String::from_utf8_lossy(&output.stderr).into_owned())
                    }
                })?,
        ),
        _ => return Err(format!("resource not found: {uri}")),
    };
    Ok(json!({"contents": [{"uri": uri, "mimeType": mime_type, "text": text}]}))
}

fn http_get(address: &str, path: &str) -> Result<String, String> {
    http_request(address, "GET", path, None)
}

fn http_post(address: &str, path: &str, body: &Value) -> Result<String, String> {
    http_request(address, "POST", path, Some(body.to_string()))
}

fn http_request(
    address: &str,
    method: &str,
    path: &str,
    body: Option<String>,
) -> Result<String, String> {
    let mut stream = TcpStream::connect(address)
        .map_err(|error| format!("agent API unavailable at {address}: {error}"))?;
    let body = body.unwrap_or_default();
    let headers = if body.is_empty() {
        String::new()
    } else {
        format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\n",
            body.len()
        )
    };
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n{headers}\r\n{body}"
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|error| format!("agent API write failed: {error}"))?;
    let mut response = String::new();
    std::io::Read::read_to_string(&mut stream, &mut response)
        .map_err(|error| format!("agent API read failed: {error}"))?;
    let (headers, body) = response
        .split_once("\r\n\r\n")
        .ok_or_else(|| "invalid agent API response".to_string())?;
    let status = headers
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("500");
    if status != "200" {
        return Err(body.to_string());
    }
    Ok(body.to_string())
}

fn required_string(request: &Value, pointer: &str) -> Result<String, String> {
    request
        .pointer(pointer)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("missing string parameter: {pointer}"))
}

fn required_string_value(value: &Value, key: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("missing string argument: {key}"))
}

fn json_rpc_error(id: Value, code: i64, message: impl Into<String>) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message.into()}})
}

fn error_code(message: &str) -> i64 {
    if message.starts_with("method not found") {
        -32601
    } else if message.starts_with("missing")
        || message.starts_with("unknown tool")
        || message.starts_with("resource not found")
    {
        -32602
    } else {
        -32603
    }
}

fn write_response(stdout: &mut impl Write, response: Value) {
    if let Err(error) = writeln!(stdout, "{response}") {
        eprintln!("MCP stdout error: {error}");
    }
    let _ = stdout.flush();
}
