# Agent API

The agent API is intentionally small and transport-independent. It exposes a narrow semantic contract for app inspection, state retrieval, and command execution. The core contract lives in the Rust application layer, and any future HTTP, JSON-RPC, or MCP adapter should call into the same interface.

## Shared service interface

`agent-api` exposes the transport-independent `AgentService` trait. It is the
single semantic boundary for all adapters and keeps HTTP, MCP, and future
transports on the same application behavior:

```rust
get_state()
inspect_ui()
execute_command(command)
execute_ui_action(action)
```

`AgentApi<S>` is the default implementation backed by the application
service. Transport adapters should depend on `AgentService` rather than
duplicating state or command handling.

## HTTP-like semantic endpoints
- `GET /health`
- `GET /agent/state`
- `GET /agent/ui`
- `POST /agent/command`
- `POST /agent/ui/action`

The desktop binary binds the development-only HTTP transport to
`127.0.0.1:8080` when built with the default `agent-api` feature. A production
build can disable it with `cargo build -p desktop --no-default-features`; the
UI remains available without opening an automation listener. It is not enabled
as a public service. Use
`scripts/agent state` and `scripts/agent ui` to inspect before acting, then
`scripts/agent command submit value` for application intent or
`scripts/agent set main.input value` and `scripts/agent click main.submit` to
exercise the human-facing UI path.

Stable UI action errors use an application error envelope with a machine-readable
`code` and a diagnostic `message`. Application commands and UI actions share the
same `AgentApi` runtime, so they cannot diverge in business behavior.

## MCP adapter

Run `scripts/mcp` inside the DevContainer to start a development-only
Model Context Protocol server over stdin/stdout. It proxies the running
loopback agent API, so it never creates a second application state or duplicates
business logic.

The adapter implements JSON-RPC methods:

- `initialize`
- `tools/list`
- `tools/call`
- `resources/list`
- `resources/read`
- `ping`

Tools are `app_state`, `ui_inspect`, `app_command`, `ui_action`, and
`ui_screenshot`. Resources are `app://state`, `ui://tree`, and `app://logs`.
Screenshot names are restricted to safe filename characters.
The recommended testing loop is `ui_inspect` -> `ui_action` or `app_command` ->
`app_state` -> `ui_screenshot`. MCP clients should use application commands for
intent and semantic UI actions only when testing the human-facing UI path.

The adapter binds only to the local API at `127.0.0.1:8080` by default. Set
`AGENT_API_ADDR` only to another explicitly trusted local endpoint. It is
included in the development crate binary and is not part of the
`--no-default-features` desktop production-shaped build.

## Security model

This is a development-only automation surface. Keep the HTTP API loopback
bound, launch MCP only from a trusted agent process, and do not point
`AGENT_API_ADDR` at a remote endpoint unless that endpoint provides its own
authentication and transport security. Logs and screenshots can contain
application-visible data; application code must not place credentials, tokens,
or personal data in them.

MCP clients should launch the server with the repository as the working
directory:

```json
{
  "command": "/workspace/scripts/mcp",
  "cwd": "/workspace",
  "env": { "AGENT_API_ADDR": "127.0.0.1:8080" }
}
```

JSON-RPC errors use standard protocol codes (`-32700` parse error, `-32601`
method not found, `-32602` invalid parameters, and `-32603` internal error).
JSON-producing tools also include `structuredContent` alongside their readable
text content.

The desktop adapter synchronizes semantic agent actions back into the visible
Slint window on the UI event loop. This means `set_value`, `click`, and
application commands can be followed by visual verification without creating a
second UI state.

## Example state
```json
{
  "screen": "main",
  "status": "ready",
  "busy": false
}
```

## Example UI inspection
```json
{
  "screen": "main",
  "elements": [
    { "id": "main.input", "role": "textbox", "enabled": true, "value": "" },
    { "id": "main.submit", "role": "button", "enabled": false, "accessible_label": "Submit" },
    { "id": "main.status", "role": "status", "enabled": true, "value": "ready" }
  ]
}
```
