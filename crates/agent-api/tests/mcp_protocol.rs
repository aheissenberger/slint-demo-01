use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Stdio};
use std::thread;

fn run_mcp(input: &str, address: &str) -> Vec<Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_agent-mcp"))
        .env("AGENT_API_ADDR", address)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn MCP binary");
    child
        .stdin
        .take()
        .expect("MCP stdin")
        .write_all(input.as_bytes())
        .expect("write MCP requests");
    let output = child.wait_with_output().expect("wait for MCP binary");
    assert!(output.status.success());
    String::from_utf8(output.stdout)
        .expect("MCP output is UTF-8")
        .lines()
        .map(|line| serde_json::from_str(line).expect("valid JSON-RPC response"))
        .collect()
}

#[test]
fn stdio_protocol_initializes_and_ignores_notifications() {
    let responses = run_mcp(
        "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n\
         {\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}\n\
         {\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"ping\"}\n",
        "127.0.0.1:1",
    );

    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0]["id"], 1);
    assert_eq!(
        responses[0]["result"]["serverInfo"]["name"],
        "slint-agent-desktop"
    );
    assert_eq!(
        responses[1],
        json!({"jsonrpc": "2.0", "id": 2, "result": {}})
    );
}

#[test]
fn stdio_protocol_proxies_tool_call_to_agent_service() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock agent API");
    let address = listener.local_addr().expect("mock address").to_string();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept MCP request");
        let mut request = [0_u8; 4096];
        let size = stream.read(&mut request).expect("read MCP request");
        let request = String::from_utf8_lossy(&request[..size]);
        assert!(request.starts_with("GET /agent/state HTTP/1.1"));
        let body = r#"{"screen":"main","status":"ready","busy":false}"#;
        write_http_response(&mut stream, body);
    });

    let responses = run_mcp(
        "{\"jsonrpc\":\"2.0\",\"id\":\"state\",\"method\":\"tools/call\",\"params\":\
         {\"name\":\"app_state\",\"arguments\":{}}}\n",
        &address,
    );
    server.join().expect("mock agent API server");

    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0]["id"], "state");
    assert_eq!(
        responses[0]["result"]["content"][0]["text"],
        r#"{"screen":"main","status":"ready","busy":false}"#
    );
    assert_eq!(responses[0]["result"]["isError"], false);
}

fn write_http_response(stream: &mut TcpStream, body: &str) {
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
    .expect("write mock response");
}
