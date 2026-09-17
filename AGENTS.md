# AGENTS.md

This repository is designed for agentic software development inside a Linux DevContainer. The host machine is macOS + Docker Desktop + VS Code; the actual build, run, test, and UI automation happen in the container.

## Core principles
- Keep business logic in Rust domain/application layers.
- Keep the Slint UI as a thin presentation layer.
- Write all user-visible product text in German by default, including UI
  labels, dialogs, validation messages, installer text, documentation intended
  for end users, and agent-facing status/error strings. Keep internal
  identifiers, code symbols, protocol fields, and tests in their established
  language unless changing them is explicitly required.
- Expose semantic UI identifiers (`screen.element`, `dialog.element`, etc.).
- Prefer application APIs over UI automation for normal operations.
- Every UI element must be fully usable through the agent API with no
  difference in outcome from using the native rendered UI; see "Agent API
  parity (mandatory)" below.
- Keep tooling deterministic and reproducible via scripts under `scripts/`.
- When adding new crates, always use the current stable release. Verify the
  version against the registry before committing dependency manifest changes.
- Validate with `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings` and the script commands documented below.

## Commands
- `./scripts/check` — format, clippy, and static checks (includes
  `./scripts/agent-api-check`)
- `./scripts/test` — run unit and integration tests
- `./scripts/run` — launch the desktop app inside the container
- `./scripts/screenshot` — capture a UI screenshot from the virtual desktop
- `./scripts/doctor` — health-check the DevContainer environment
- `./scripts/agent logs` — stream structured logs emitted by the app and agent API
- `./scripts/agent-api-check` — statically cross-checks `agent-id`
  declarations/markers in `ui/**/*.slint` against the element/action ids in
  `crates/agent-api`; fails on drift in either direction

## Architecture
- `crates/domain` contains pure business rules and value objects.
- `crates/application` contains commands, queries, and orchestration.
- `crates/infrastructure` contains adapters (filesystem, network, OS).
- `crates/slint-demo` contains Slint binding and lifecycle logic.
- `crates/agent-api` contains agent-facing HTTP/JSON-RPC semantics.
- `ui/` contains Slint presentation files and visual theme assets.

## UI accessibility and semantics
- Use stable semantic IDs for interactive elements.
- Do not use translated text or screen coordinates for identification.
- Keep accessible roles and labels up to date whenever controls change.

## Agent API parity (mandatory)
This project exists to let a coding agent operate the application without GUI
automation (`SETUP-PROMPT.md` sections 2, 8–11, 31–36). GUI automation exists
only to verify that the human-facing UI invokes the same operations an agent
already can — never as the only way to reach a feature.

- Every interactive Slint element (menu item, dialog, button, field, popup,
  etc.) must have a corresponding entry in the `agent-api` semantic contract
  (`inspect_ui` / `ui_inspect`) and a working `execute_ui_action` handler
  (`click`, `set_value`, `get_value`, `focus`, or an equivalent semantic verb).
  Adding a new `.slint` control without a matching agent-api element/action is
  an incomplete change. Give every such element an `agent-id` — either the
  `agent-id` property (already declared on `FluentButton`/`FluentTextField`)
  or, for built-ins that cannot carry a custom property (`MenuItem`, `Text`,
  `PopupWindow`, ...), a `// agent-id: "screen.element"` comment directly
  above it — and run `./scripts/agent-api-check` (part of `./scripts/check`)
  to statically verify every `agent-id` in `ui/` has a matching id in
  `crates/agent-api`, and vice versa.
- An element that only exists while shown (a popup, dialog, or menu) must
  appear in and disappear from `inspect_ui`'s element list exactly when it is
  actually visible in the rendered UI — the semantic tree must never claim a
  state the visible window does not have, or vice versa.
- Actions performed through the agent API (`scripts/agent`, MCP tools, or the
  HTTP endpoints) must produce the exact same visible result in the running
  Slint window as the equivalent human interaction, and must do so without a
  restart or extra step. Wire this through the existing state-sync mechanism
  (see `crates/slint-demo/src/lib.rs`'s sync timer) rather than adding a
  parallel/duplicate state machine.
- Conversely, actions performed by a human in the rendered UI must be fully
  visible through `inspect_ui`/`get_state` — there must be no UI state an
  agent cannot observe.
- Never implement agent-only behavior that bypasses the same application
  layer/callback the Slint UI uses, and never implement UI-only behavior that
  has no agent-reachable equivalent.
- When verifying a UI change, prefer `scripts/agent` (`ui`, `click`, `set`,
  `get`, `state`) and `scripts/screenshot` over interactive browser/VNC
  clicking. Browser-driven interaction is acceptable only to double-check that
  a human using the real UI reaches the same state the agent API already
  proved reachable — it must not be the primary or only verification path.
- Add or update `agent-api` tests (unit and `tests/semantic_contract.rs`) for
  every new semantic element so the contract in `docs/agent-api.md` stays
  accurate and regression-tested.

## UI design rules
- Read `docs/design-system.md` and the scoped `ui/AGENTS.md` before changing a
  `.slint` file.
- Treat the Fluent 2 requirements as mandatory for Windows-targeted UI, not as
  a loose "Fluent-inspired" theme.
- Review composition as well as styling: a flat header/form, decorative card
  stack, or unused elevation token is not a Windows 11 page hierarchy.
- Reuse existing components and design tokens; do not invent arbitrary colors,
  spacing, typography, radii, or state treatments.
- Literal visual values are prohibited in both screen files and reusable
  components unless the design-system documentation records a necessary
  exception.
- Preserve semantic IDs, keyboard navigation, visible focus, and accessible labels.
- Keep business logic out of Slint and do not use emoji as icons.
- After UI changes, run `./scripts/check`, relevant tests, semantic UI
  inspection and interaction via `scripts/agent` (see "Agent API parity
  (mandatory)" above), and capture a screenshot with `scripts/screenshot`.
  Compare the result against every Fluent 2 review checklist item in
  `docs/design-system.md`; validate Windows chrome and rendering on Windows
  when the change affects either. Do not mark the task complete when a check
  is skipped or fails without a documented, tested exception.

## Safety
- Never log secrets, credentials, tokens, or personal data.
- Keep the agent API contract explicit and stable.
- Keep future MCP integration behind an internal inspection/command interface.
- `agent-api` is an opt-in Cargo feature on `slint-demo` (not part of
  `default`). A plain `cargo build`/`cargo build --release` must never bundle
  the local automation HTTP server; dev/test entry points
  (`scripts/run`, `scripts/doctor`, `.devcontainer/scripts/start-desktop.sh`)
  enable it explicitly with `--features agent-api`, and CI release workflows
  build with `--no-default-features` as a belt-and-suspenders guarantee. See
  `SETUP-PROMPT.md` section 25.
