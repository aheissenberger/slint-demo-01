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

The settings surface is reachable through
`scripts/agent click file.settings`. While it is open, select the app theme
with `settings.theme.system`, `settings.theme.light`, or
`settings.theme.dark`, then close it with `settings.close`. The same operation
is available as the `set_theme` command with a `mode` argument of `system`,
`light`, or `dark`.

The settings surface also exposes data maintenance. `settings.data.summary`
reports the storage location and object counts, `settings.data.backup`
creates a timestamped SQLite backup, `settings.data.backup-path` exposes the
last backup path, and `settings.data.reset` clears local notes/submissions and
restores default settings. The same operations are available as
`create_data_backup` and `reset_user_data` commands.

The notes AppShell is reachable without opening a dialog. Use
`scripts/agent set notes.title "Titel"` and
`scripts/agent set notes.body "Inhalt"` followed by
`scripts/agent click notes.save` to create or update a note. Existing notes are
exposed as stable visible slots `notes.item.0` through `notes.item.4`; empty
slots remain disabled. The same behavior is available through commands:
`new_note`, `select_note`, `set_note_title`, `set_note_body`, `save_note`,
`archive_note`, and `delete_note`.

Error, validation, retry, and task flows are part of the same semantic
contract. `app_state` includes:

- `ui_error` with `code`, `severity`, user text, diagnostic text, and
  recoverability.
- `validation_errors` keyed by semantic field IDs such as `main.input` and
  `notes.title`.
- `tasks`, `active_task_id`, and `can_retry` for concurrent long-running
  operations.
- `data_summary` and `last_backup_path` for data maintenance and recovery.

Recoverable errors appear as `main.error` with `main.retry` and
`main.error.dismiss`. Critical recovery-required errors appear as
`error.dialog` with `error.dialog.close`. Background task state is exposed as
`main.tasks`. The commands `retry_last_task`, `dismiss_error`, and
`cancel_task` mirror the corresponding UI actions.

Stable UI action errors use an application error envelope with a
machine-readable `code`, user-facing text, diagnostic text, and a recoverable
flag. Application commands and UI actions share the same `AgentApi` runtime, so
they cannot diverge in business behavior.

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

The desktop adapter subscribes to application state updates and forwards each
update directly to Slint with `upgrade_in_event_loop`, which wakes the UI event
loop. There is no synchronization polling timer. This means `set_value`,
`click`, and application commands can be followed by visual verification
without creating a second UI state.

Transient UI work is emitted separately from persistent state: focusing a
control and opening the native file dialog are one-shot effects. The Slint
adapter executes each effect once, so a later state update cannot repeat a
file-dialog request or steal focus.

## Shared application state and component metadata

The application store in `crates/application` is the single source of truth
for state used by both the rendered Slint window and the agent API. Human UI
callbacks and agent actions are translated into the same typed
`AppAction` values. The store publishes each new state to the Slint adapter, so
the agent API no longer owns a duplicate UI state or relies on inspecting its
own state to synchronize the window. No-op transitions do not increment the
revision or publish another update, preventing feedback when a rendered
popup reports its already-current visibility back to the store.

The agent-facing session state (dialog visibility, draft input, busy/error
status) is intentionally in-memory only: it resets to defaults on restart and
is not persisted to disk. This keeps the dev-only automation surface
predictable while still letting the UI and agent API share the same live
state. The persisted appearance preference (`theme_mode`) is the one
exception: it is loaded from and saved to the repository (see
`docs/architecture.md`), so it survives a restart.

Reusable components declare their agent-facing capabilities explicitly through
Slint properties, and the agent adapter keeps the corresponding stable
semantic metadata registry:

```slint
in property <string> agent-id: "";
in property <string> agent-role: "button";
in property <string> agent-actions: "click,focus";
```

`agent-id` is assigned at the screen instance, not fixed by the reusable
component. `agent-role` and `agent-actions` describe the component contract;
the runtime `inspect_ui` response supplies current values, enabled state, and
visibility. The registry is covered by semantic contract tests and is used to
build `AgentElement` responses, rather than repeating roles and actions in
each runtime element literal. This keeps static component metadata separate
from dynamic application state and avoids duplicate IDs when a component is
reused.

## Example state
```json
{
  "screen": "main",
  "status": "bereit",
  "busy": false,
  "error": null,
  "ui_error": null,
  "validation_errors": [],
  "tasks": [],
  "active_task_id": null,
  "can_retry": false,
  "theme_mode": "system",
  "notes": [],
  "selected_note_id": null,
  "note_title": "",
  "note_body": "",
  "data_summary": "memory · 0 aktive Notizen · 0 archivierte Notizen · 0 Übermittlungen",
  "last_backup_path": null
}
```

## Example UI inspection
```json
{
  "screen": "main",
  "elements": [
    { "id": "main.input", "role": "textbox", "enabled": true, "value": "" },
    { "id": "notes.title", "role": "textbox", "enabled": true, "value": "", "accessible_label": "Notiztitel" },
    { "id": "notes.save", "role": "button", "enabled": false, "accessible_label": "Speichern" },
    { "id": "notes.item.0", "role": "button", "enabled": false, "value": "", "accessible_label": "Leerer Notizplatz" },
    { "id": "main.file-picker", "role": "button", "enabled": true, "value": "", "accessible_label": "Datei auswählen" },
    { "id": "main.selected-file", "role": "status", "enabled": true, "value": "", "accessible_label": "Ausgewählter Dateipfad" },
    { "id": "main.submit", "role": "button", "enabled": false, "accessible_label": "Senden" },
    { "id": "main.status", "role": "status", "enabled": true, "value": "bereit", "accessible_label": "Anwendungsstatus" },
    { "id": "main.tasks", "role": "status", "enabled": true, "value": "", "accessible_label": "Hintergrundvorgänge" },
    { "id": "file.settings", "role": "menuitem", "enabled": true, "accessible_label": "Einstellungen" },
    { "id": "help.about", "role": "menuitem", "enabled": true, "accessible_label": "Über" }
  ]
}
```

## Settings and appearance

The German `Datei` menu contains `Einstellungen`. It opens a modal Fluent
settings surface with the `Darstellung` section and an `App-Design` radio
group:

- `settings.theme.system` follows the operating-system color scheme.
- `settings.theme.light` forces the light color scheme.
- `settings.theme.dark` forces the dark color scheme.

The selected value is exposed as `theme_mode` in application state. Unlike
the rest of the ephemeral agent-facing state, the theme selection is
persisted through the repository and is restored on the next application
start; it only falls back to `system` when no settings have been saved yet
or the persisted value cannot be parsed.

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
(part of `./scripts/check`) compiles the UI, extracts structured JSON containing
each id, role, action set, declaration kind, path, and line, and fails on
duplicate concrete ids. It then compares those declarations with
`inspect_ui`, action handlers, and the Rust component metadata registry.
This keeps the semantic contract accurate without requiring runtime
introspection of Slint's internal AccessKit accessibility tree, which is not
part of the stable public `slint` crate API.
