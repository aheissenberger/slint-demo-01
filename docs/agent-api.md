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
`127.0.0.1:8080` when built with the opt-in `agent-api` feature. A production
build omits it by default; `cargo build -p slint-demo --no-default-features`
makes that posture explicit. The UI remains available without opening an
automation listener. It is not enabled as a public service. Use
`scripts/agent state` and `scripts/agent ui` to inspect before acting, then
`scripts/agent command submit value` for application intent or
`scripts/agent set main.input value` and `scripts/agent click main.submit` to
exercise the human-facing UI path. In development, configure the deterministic
file-picker result with `scripts/agent set main.file-picker /path/to/file`,
then select it with `scripts/agent click main.file-picker`. The resulting path
is exposed as `main.selected-file` and appears beneath the button. This
development behavior deliberately bypasses the system dialog because it is not
available in the VNC automation environment; builds without `agent-api` open
the platform-native picker instead.

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
`AGENT_API_ADDR` only to another explicitly trusted local endpoint. It ships
behind the opt-in `agent-api` Cargo feature on `slint-demo`, which is **not**
part of `default`: a plain `cargo build`/`cargo build --release` never bundles
this HTTP server. Development and test entry points
(`scripts/run`, `scripts/doctor`, `.devcontainer/scripts/start-desktop.sh`)
enable it explicitly with `--features agent-api`; CI release workflows
additionally build with `--no-default-features` for the shipped desktop
binaries.

## Security model

This is a development-only automation surface, gated behind the opt-in
`agent-api` Cargo feature (see above) so a production binary never
unexpectedly exposes a local automation server. Keep the HTTP API loopback
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
  "status": "bereit",
  "busy": false
}
```

## Example UI inspection
```json
{
  "screen": "main",
  "elements": [
    { "id": "main.input", "role": "textbox", "enabled": true, "value": "" },
    { "id": "main.file-picker", "role": "button", "enabled": true, "value": "/workspace/Cargo.toml", "accessible_label": "Datei auswählen" },
    { "id": "main.selected-file", "role": "status", "enabled": true, "value": "", "accessible_label": "Ausgewählter Dateipfad" },
    { "id": "main.submit", "role": "button", "enabled": false, "accessible_label": "Senden" },
    { "id": "main.status", "role": "status", "enabled": true, "value": "bereit", "accessible_label": "Anwendungsstatus" },
    { "id": "help.about", "role": "menuitem", "enabled": true, "accessible_label": "Über" }
  ]
}
```

## About menu and dialog

The German "Hilfe" menu exposes an "Über" (About) command required by the
Slint Royalty-free License to disclose the use of Slint from the
application's top-level menu. `help.about` is always present in the semantic
tree; clicking it (`ui_action` / `scripts/agent click help.about`) opens the
dialog, which then adds `about.dialog` (role `dialog`) and `about.close`
(role `button`) to `ui_inspect` until it is closed
(`scripts/agent click about.close`). The desktop adapter keeps the visible
popup and the semantic tree in sync in both directions, so the dialog can be
opened, inspected, and closed entirely through the agent interface.

## Keeping the UI and agent-api in sync

Every element id used above traces back to an `agent-id` declared in
`ui/**/*.slint` — either the `agent-id` property on reusable components
(`FluentButton`, `FluentTextField`) or a `// agent-id: "screen.element"`
comment marker directly above a built-in element that cannot carry a custom
property (`MenuItem`, `Text`, `PopupWindow`). `./scripts/agent-api-check`
(part of `./scripts/check`) statically parses both sides and fails the build
if a UI `agent-id` has no matching `inspect_ui` element, or if `agent-api`
references an id no longer declared in the UI. This keeps the semantic
contract accurate without requiring runtime introspection of Slint's internal
AccessKit accessibility tree, which is not part of the stable public `slint`
crate API.
