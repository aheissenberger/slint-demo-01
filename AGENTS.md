# AGENTS.md

This repository is designed for agentic software development inside a Linux DevContainer. The host machine is macOS + Docker Desktop + VS Code; the actual build, run, test, and UI automation happen in the container.

## Core principles
- Keep business logic in Rust domain/application layers.
- Keep the Slint UI as a thin presentation layer.
- Expose semantic UI identifiers (`screen.element`, `dialog.element`, etc.).
- Prefer application APIs over UI automation for normal operations.
- Keep tooling deterministic and reproducible via scripts under `scripts/`.
- Validate with `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings` and the script commands documented below.

## Commands
- `./scripts/check` — format, clippy, and static checks
- `./scripts/test` — run unit and integration tests
- `./scripts/run` — launch the desktop app inside the container
- `./scripts/screenshot` — capture a UI screenshot from the virtual desktop
- `./scripts/doctor` — health-check the DevContainer environment
- `./scripts/agent logs` — stream structured logs emitted by the app and agent API

## Architecture
- `crates/domain` contains pure business rules and value objects.
- `crates/application` contains commands, queries, and orchestration.
- `crates/infrastructure` contains adapters (filesystem, network, OS).
- `crates/desktop` contains Slint binding and lifecycle logic.
- `crates/agent-api` contains agent-facing HTTP/JSON-RPC semantics.
- `ui/` contains Slint presentation files and visual theme assets.

## UI accessibility and semantics
- Use stable semantic IDs for interactive elements.
- Do not use translated text or screen coordinates for identification.
- Keep accessible roles and labels up to date whenever controls change.

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
  inspection and interaction when available, and capture a screenshot. Compare
  the result against every Fluent 2 review checklist item in
  `docs/design-system.md`; validate Windows chrome and rendering on Windows
  when the change affects either. Do not mark the task complete when a check
  is skipped or fails without a documented, tested exception.

## Safety
- Never log secrets, credentials, tokens, or personal data.
- Keep the agent API contract explicit and stable.
- Keep future MCP integration behind an internal inspection/command interface.
