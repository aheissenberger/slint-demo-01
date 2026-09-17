# Task: Create an agent-first Rust + Slint desktop development environment

Create a new production-quality Rust desktop application using **Slint**. Development will take place on **macOS with Docker Desktop and VS Code**, but ALL development tools, compilation, testing, agent commands, and application execution must happen inside a **Linux VS Code DevContainer**.

The application must be viewable and interactable from macOS through **VNC/noVNC**, while the actual application runs inside the DevContainer.

Design the repository specifically for **agentic development** with Claude Code, Codex, Copilot, or similar coding agents.

Do not merely make a minimal demo. Establish a clean foundation for a larger production application.

---

# 1. Primary goals

The environment must provide:

- Rust stable toolchain
- Cargo
- rustfmt
- clippy
- rust-analyzer
- Slint
- Slint compiler/tooling
- Slint VS Code support
- Linux desktop dependencies
- X11/Xvfb-based virtual display
- lightweight window manager
- VNC server
- noVNC browser access
- deterministic build/test/check commands
- unit tests
- integration tests
- UI smoke tests
- semantic application inspection
- semantic application operations
- future MCP integration
- screenshots usable by coding agents
- structured logs
- clean separation of UI and application logic

The DevContainer must be self-contained.

Do not require Rust, Slint, Node, VNC, GUI libraries, or build tools on macOS.

The only host requirements should be:

- macOS
- Docker Desktop
- VS Code
- VS Code Dev Containers extension

---

# 2. Architecture principle

Use this architecture:

```text
                         ┌─────────────────┐
                         │   Coding Agent  │
                         └────────┬────────┘
                                  │
             ┌────────────────────┼────────────────────┐
             │                    │                    │
             ▼                    ▼                    ▼
        source editing       semantic API         UI automation
             │                    │                    │
             ▼                    ▼                    ▼
       cargo/slint check    application layer       Slint UI
             │                    │                    │
             └────────────────────┴──────────┬─────────┘
                                             │
                                             ▼
                                      application core
```

The GUI must NOT contain business logic.

The agent must NOT need GUI automation to perform normal application operations.

Expose application operations separately from UI operations.

GUI automation exists primarily to verify that the human-facing interface correctly invokes those operations.

---

# 3. Repository structure

Create approximately:

```text
.
├── .devcontainer/
│   ├── devcontainer.json
│   ├── Dockerfile
│   ├── docker-compose.yml
│   ├── scripts/
│   │   ├── start-desktop.sh
│   │   ├── start-vnc.sh
│   │   ├── healthcheck.sh
│   │   └── post-create.sh
│   └── supervisor/
│       └── supervisord.conf
│
├── .vscode/
│   ├── extensions.json
│   ├── settings.json
│   ├── tasks.json
│   └── launch.json
│
├── crates/
│   ├── domain/
│   ├── application/
│   ├── infrastructure/
│   ├── agent-api/
│   └── desktop/
│
├── ui/
│   ├── app.slint
│   ├── components/
│   ├── screens/
│   └── theme/
│
├── tests/
│   ├── integration/
│   └── ui/
│
├── scripts/
│   ├── check
│   ├── test
│   ├── run
│   ├── screenshot
│   ├── doctor
│   └── agent
│
├── docs/
│   ├── architecture.md
│   ├── agent-api.md
│   ├── development.md
│   └── testing.md
│
├── artifacts/
│   └── screenshots/
│
├── AGENTS.md
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── README.md
└── .gitignore
```

Use a Cargo workspace.

Keep crates small and boundaries explicit.

---

# 4. Rust architecture

Create these logical layers.

## domain

Pure Rust.

Contains:

- domain entities
- value objects
- domain rules
- domain errors

Must NOT depend on:

- Slint
- GUI libraries
- filesystem
- networking
- databases
- agent protocol

It should be straightforward to unit-test.

## application

Contains:

- application state
- commands
- queries
- use cases
- service interfaces/ports

It may depend on `domain`.

It must NOT depend on Slint.

All meaningful application operations should be accessible through this layer.

## infrastructure

Contains adapters for:

- filesystem
- HTTP
- persistence
- OS integration
- external services

Keep these behind interfaces where practical.

## desktop

The Slint adapter.

Responsibilities:

- create Slint application
- translate application state into UI properties/models
- translate Slint callbacks into application commands
- manage UI lifecycle

It must NOT implement business rules.

## agent-api

Provide a semantic interface into the application.

Initially implement this as a simple local JSON/JSON-RPC or HTTP interface.

Architect it so an MCP server can later be added without changing the application layer.

---

# 5. Slint rules

Use Slint for:

- presentation
- layouts
- visual components
- theme
- animations
- simple presentation-only state
- callbacks
- UI bindings

Use Rust for:

- business rules
- persistence
- filesystem
- HTTP
- authentication
- cryptography
- background work
- application state
- validation that has business meaning

Avoid large global Slint state objects.

The authoritative application state belongs in Rust.

Prefer:

```text
Rust state
    │
    ├── state -> Slint properties/models
    │
    └── callbacks -> Rust commands
```

over bidirectional implicit state spread across many Slint components.

---

# 6. Semantic UI identifiers

Design reusable components so important interactive elements have stable semantic identifiers.

For example, conceptually:

```text
agent-id = "main.submit"
agent-id = "settings.server-url"
agent-id = "dialog.confirm"
```

Do NOT identify elements by:

- screen coordinates
- translated visible text
- implementation-generated IDs
- component position

IDs must be stable across layout and localization changes.

Use hierarchical names:

```text
screen.element
screen.section.element
dialog.element
```

Document the naming convention.

---

# 7. Accessibility

Accessibility is mandatory.

For every meaningful interactive component provide appropriate:

- accessible role
- accessible label
- accessible description where necessary
- keyboard focus behavior
- keyboard operation

The semantic accessibility information should be designed so that it can eventually also contribute to the agent inspection tree.

Do not treat accessibility metadata as optional decoration.

---

# 8. Agent API

Implement a small development-only semantic agent API.

It should expose at least:

```text
GET /health

GET /agent/state

GET /agent/ui

POST /agent/command

POST /agent/ui/action
```

Equivalent JSON-RPC is acceptable if cleaner.

`/agent/state` should return meaningful application state.

Example:

```json
{
  "screen": "main",
  "status": "ready",
  "busy": false
}
```

`/agent/ui` should return a semantic representation such as:

```json
{
  "screen": "main",
  "elements": [
    {
      "id": "main.input",
      "role": "textbox",
      "enabled": true,
      "value": ""
    },
    {
      "id": "main.submit",
      "role": "button",
      "enabled": false
    }
  ]
}
```

Do not expose arbitrary internal Rust memory/state.

Expose a deliberate, stable semantic contract.

---

# 9. Application commands

The agent API should expose application-level commands separately from UI actions.

Example:

```json
POST /agent/command

{
  "command": "example_operation",
  "arguments": {
    "value": "..."
  }
}
```

This invokes the SAME application layer that the Slint UI uses.

Never implement duplicate business logic specifically for agents.

Architecture:

```text
Human
  │
Slint
  │
  └──────────────┐
                 ▼
          Application Layer
                 ▲
  ┌──────────────┘
  │
Agent API
  │
Agent
```

---

# 10. UI actions

The agent UI API should eventually support semantic operations such as:

```text
click(id)
set_value(id, value)
get_value(id)
focus(id)
activate(id)
```

Do not use coordinate-based clicking unless absolutely unavoidable.

Return useful structured errors:

```json
{
  "error": "element_disabled",
  "element": "main.submit"
}
```

rather than generic 500 errors.

---

# 11. MCP-ready design

Do not require MCP for the first implementation, but make `agent-api` transport-independent.

Design an internal Rust interface approximately equivalent to:

```text
inspect_state()
inspect_ui()
execute_command(command)
execute_ui_action(action)
```

A future MCP adapter should be able to expose:

Resources:

```text
app://state
ui://tree
app://logs
```

Tools:

```text
app_command
ui_click
ui_set_value
ui_get_value
ui_focus
ui_screenshot
```

without changing the application core.

Document this extension point in `docs/agent-api.md`.

---

# 12. Structured logging

Use Rust `tracing`.

Logs should be available both human-readable and, where useful, JSON.

Include useful fields such as:

```text
operation
component
command
duration
result
error
```

Never log:

- passwords
- tokens
- credentials
- sensitive personal data

Agents should be able to inspect logs easily.

Provide:

```bash
scripts/agent logs
```

or equivalent.

---

# 13. DevContainer GUI environment

The Linux DevContainer must run the Slint GUI application.

Use a virtual desktop stack approximately:

```text
Slint application
       │
       ▼
      X11
       │
       ▼
      Xvfb
       │
       ├── lightweight WM
       │
       ▼
     x11vnc
       │
       ▼
      noVNC
       │
       ▼
macOS browser
```

Prefer a minimal window manager such as Openbox or Fluxbox.

Do not install a full GNOME/KDE desktop unless necessary.

Use a predictable display, for example:

```text
DISPLAY=:1
```

Use a predictable resolution such as:

```text
1440x900x24
```

Make the resolution configurable.

---

# 14. noVNC

Expose noVNC from the DevContainer on a predictable forwarded port, preferably:

```text
6080
```

The developer should be able to:

1. open the repository in VS Code
2. reopen it in the DevContainer
3. wait for services to start
4. run `scripts/run`
5. open the forwarded noVNC port
6. see and interact with the Slint application

Document the exact workflow.

Do not require manual VNC setup after container creation.

---

# 15. Service supervision

Use a lightweight supervisor or robust startup scripts for:

- Xvfb
- window manager
- x11vnc
- noVNC/websockify

Processes should:

- restart where appropriate
- log clearly
- fail visibly
- not silently hide errors

Provide:

```bash
scripts/doctor
```

which verifies at least:

```text
Rust toolchain
cargo
Slint compiler/tooling
DISPLAY
Xvfb
window manager
VNC
noVNC
agent API
application process if running
```

Output concise PASS/FAIL results and actionable errors.

---

# 16. Screenshots

Implement:

```bash
scripts/screenshot
```

It must capture the virtual X display and write PNG files under:

```text
artifacts/screenshots/
```

Use deterministic timestamped or explicitly named filenames.

Example:

```bash
scripts/screenshot login-screen
```

produces something similar to:

```text
artifacts/screenshots/login-screen.png
```

Agents must be able to create screenshots without interacting with macOS.

---

# 17. Headless operation

The application architecture should permit as much testing as possible WITHOUT VNC.

Use:

```text
unit tests
      ↓
application tests
      ↓
agent semantic tests
      ↓
GUI tests
```

Do not make every test launch a graphical application.

VNC is for visual development/debugging, not the primary test mechanism.

---

# 18. Testing pyramid

Establish four levels.

## Level 1 — domain unit tests

Fast Rust tests.

No Slint.

No network.

No filesystem unless explicitly testing an adapter.

## Level 2 — application tests

Test commands/use cases against fake adapters.

No GUI.

## Level 3 — semantic agent tests

Start application services and exercise:

```text
agent command
agent state
agent semantic UI
```

without relying on pixels.

## Level 4 — GUI smoke/E2E tests

Run the actual Slint application under Xvfb.

Test only critical workflows.

Capture screenshots on failures.

---

# 19. Golden screenshots

Prepare the structure for optional screenshot regression testing.

Do NOT make screenshot equality the primary testing mechanism.

Semantic assertions should be preferred.

Screenshots are useful for:

- visual regression
- agent inspection
- debugging failed E2E tests
- reviewing layouts

Allow tolerance for rendering differences where appropriate.

---

# 20. Standard agent commands

Provide a very small, memorable command surface.

These commands MUST work from the repository root inside the DevContainer:

```bash
scripts/check
scripts/test
scripts/run
scripts/screenshot
scripts/doctor
scripts/agent
```

`scripts/check` should perform approximately:

```text
cargo fmt --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
Slint validation
```

`scripts/test` should run all normal non-expensive tests.

Provide an explicit command for E2E tests rather than automatically making every normal test launch the GUI.

---

# 21. Agent command helper

Make:

```bash
scripts/agent
```

provide convenient operations such as:

```bash
scripts/agent health
scripts/agent state
scripts/agent ui
scripts/agent command <name> [args]
scripts/agent click <id>
scripts/agent set <id> <value>
scripts/agent screenshot [name]
scripts/agent logs
```

The exact implementation can use curl, a small Rust CLI, or another lightweight mechanism.

Prefer a small Rust CLI if this provides stronger typing and maintainability.

---

# 22. AGENTS.md

Create a high-quality `AGENTS.md`.

It should tell coding agents:

## Before making changes

Read:

```text
AGENTS.md
docs/architecture.md
```

Read additional documentation relevant to the task.

## Architecture rules

Never:

- put business logic in `.slint`
- access infrastructure directly from `.slint`
- duplicate application logic in agent endpoints
- use coordinates when a semantic UI ID exists
- add dependencies without explaining why
- expose secrets through agent APIs
- disable compiler/linter errors merely to make checks pass

## Preferred workflow

For every meaningful change:

```text
1. inspect existing architecture
2. make smallest coherent change
3. run scripts/check
4. run relevant tests
5. use semantic inspection when UI changed
6. launch application if necessary
7. capture screenshot for meaningful visual changes
8. fix warnings/errors
```

Agents should not declare a task complete if `scripts/check` fails.

---

# 23. Agent UI workflow

Document this preferred UI-development loop:

```text
edit .slint
      ↓
Slint/compiler validation
      ↓
cargo check
      ↓
launch application
      ↓
agent semantic UI inspection
      ↓
semantic interaction
      ↓
state assertion
      ↓
screenshot
```

Visual inspection should supplement semantic inspection, not replace it.

---

# 24. Determinism

Make the development environment deterministic where practical.

Pin:

- Rust toolchain
- important system dependencies where practical
- Slint crate version
- DevContainer base image version

Commit:

```text
Cargo.lock
```

Avoid unbounded `latest` dependencies where they can affect reproducibility.

---

# 25. Development/production separation

The semantic agent API is primarily a development/testing capability.

Design it so production builds can disable it using a Cargo feature such as:

```text
agent-api
```

or equivalent.

A production binary must not unexpectedly expose a local automation server.

Document the security model.

Default to binding development agent endpoints to:

```text
127.0.0.1
```

inside the appropriate environment.

Do not expose them publicly.

---

# 26. Container networking/security

Only expose ports needed for development.

Expected ports might include:

```text
6080  noVNC
<agent-port> development agent API
```

Use VS Code DevContainer `forwardPorts`.

Do not bind sensitive development services unnecessarily to all host interfaces.

Do not bake credentials into the image.

---

# 27. Cross-platform target strategy

Development runtime:

```text
macOS
  ↓
Docker Desktop
  ↓
Linux DevContainer
  ↓
Linux Slint binary
```

Production targets:

```text
Windows x86_64
Linux x86_64
```

Structure the code so OS-specific implementations live in infrastructure/platform adapters.

Do not scatter:

```rust
#[cfg(target_os = "...")]
```

through domain/application code.

Centralize platform differences.

Document how Windows release binaries will eventually be built.

Do NOT claim that the Linux container can trivially produce a fully validated Windows GUI binary unless the required cross-compilation/toolchain setup has actually been implemented and tested.

Prefer CI with native Windows runners for final Windows builds and tests.

---

# 28. VS Code

Configure useful VS Code extensions for the DevContainer, including:

- rust-analyzer
- CodeLLDB
- Slint extension
- TOML support if needed

Configure:

- format on save
- rust-analyzer
- Clippy/checking
- Slint files
- useful tasks

Add VS Code tasks approximately for:

```text
Check
Test
Run App
Run E2E
Take Screenshot
Doctor
```

Make sure these execute INSIDE the DevContainer.

---

# 29. Debugging

Configure Rust debugging with CodeLLDB where practical.

The GUI process runs inside the DevContainer using:

```text
DISPLAY=:1
```

The developer must be able to set Rust breakpoints from VS Code while viewing the GUI through noVNC.

Document any limitations.

---

# 30. Initial application

Create a small but architecturally representative demo rather than just "Hello World".

The main screen should contain:

- heading
- text input
- action button
- status text

Behavior:

```text
empty input
    ↓
button disabled

input entered
    ↓
button enabled

click button
    ↓
application command
    ↓
application state changes
    ↓
UI status updates
```

The validation/state transition must be implemented in the application layer rather than duplicated in Slint.

Expose the same operation through the agent API.

This should prove:

```text
Slint -> application
Agent -> application
application -> state
state -> Slint
```

---

# 31. Demonstrate semantic inspection

The initial demo must make this work:

```bash
scripts/agent ui
```

and return something approximately like:

```json
{
  "screen": "main",
  "elements": [
    {
      "id": "main.input",
      "role": "textbox",
      "enabled": true
    },
    {
      "id": "main.submit",
      "role": "button",
      "enabled": false
    },
    {
      "id": "main.status",
      "role": "status"
    }
  ]
}
```

After setting the input:

```bash
scripts/agent set main.input "Test"
```

inspection should show the corresponding semantic change.

---

# 32. Important design improvement: separate intent from mechanics

Agent commands should express intent whenever possible.

Prefer:

```text
app_command("submit_example", ...)
```

over:

```text
click at x=482 y=317
```

Use GUI actions when testing GUI behavior.

Use application commands when operating the application.

This distinction must be documented prominently.

---

# 33. Important design improvement: observable state

Every asynchronous operation should expose meaningful state such as:

```text
idle
running
success
error
```

Agents must not have to guess whether work completed by sleeping arbitrary amounts.

Provide observable status or completion conditions.

Prefer:

```text
wait until state == success
```

over:

```text
sleep 5
```

If a wait helper is implemented, give it a timeout and useful failure diagnostics.

---

# 34. Important design improvement: error contracts

Use structured application errors.

For example:

```json
{
  "code": "invalid_input",
  "message": "Input is required"
}
```

Agents should be able to reason about stable error codes.

Do not make agents parse arbitrary human-readable strings to understand application state.

---

# 35. Important design improvement: machine-readable output

Agent-facing scripts should support machine-readable output.

Where useful support:

```bash
--json
```

For example:

```bash
scripts/doctor --json
scripts/agent state --json
```

Human-readable output should remain the default where appropriate.

Exit codes must correctly indicate success/failure.

---

# 36. Important design improvement: inspect before acting

The agent workflow should encourage:

```text
inspect
   ↓
understand
   ↓
act
   ↓
inspect again
   ↓
assert
```

not blind interaction.

UI actions should report the resulting relevant state where practical.

---

# 37. Important design improvement: diagnostics bundle

Provide a command such as:

```bash
scripts/agent diagnostics
```

which gathers useful NON-SECRET debugging information:

```text
tool versions
application status
agent API health
current semantic state
current semantic UI tree
recent logs
DISPLAY status
VNC status
screenshot
```

Write the result under:

```text
artifacts/diagnostics/
```

This should make it easy for a coding agent to diagnose a failed GUI run without asking the developer to manually inspect several systems.

Redact secrets.

---

# 38. Documentation

README should include a concise quick start:

```text
1. Install Docker Desktop
2. Install VS Code
3. Install Dev Containers extension
4. Clone repository
5. Reopen in Container
6. scripts/doctor
7. scripts/run
8. open forwarded port 6080
```

`docs/development.md` should explain the complete environment.

`docs/architecture.md` should explain boundaries.

`docs/agent-api.md` should explain semantic operations and future MCP support.

`docs/testing.md` should explain the testing pyramid.

---

# 39. Implementation approach

Implement this incrementally.

Recommended order:

```text
1. Cargo workspace
2. basic Slint application
3. DevContainer
4. Xvfb
5. window manager
6. VNC/noVNC
7. scripts/run
8. scripts/screenshot
9. scripts/doctor
10. domain/application separation
11. agent-api abstraction
12. development transport
13. scripts/agent
14. semantic UI model
15. tests
16. VS Code tasks/debugging
17. AGENTS.md
18. documentation
```

After each major stage, actually run the relevant commands.

Do not generate dozens of files without testing them.

---

# 40. Verification requirements

Before considering the bootstrap complete, verify inside the DevContainer:

```bash
scripts/doctor
scripts/check
scripts/test
scripts/run
```

Then verify:

```bash
scripts/agent health
scripts/agent state
scripts/agent ui
```

Exercise the demo using both:

```text
application-level command
```

and:

```text
semantic UI interaction
```

Capture a screenshot.

Verify that noVNC displays the running Slint application.

Run Clippy.

Run tests.

Report any limitation that could not actually be verified.

---

# 41. Do not over-engineer

This is a foundation, not a framework project.

Do not introduce:

- Kubernetes
- databases
- message brokers
- elaborate DI frameworks
- unnecessary web frontend frameworks
- Electron
- Tauri
- GTK application framework
- unnecessary Node runtime

unless a concrete requirement makes one necessary.

For noVNC tooling, system packages or minimal supporting utilities are fine.

The application itself should remain:

```text
Rust
+
Slint
```

---

# 42. Definition of done

The project is ready when a developer on macOS can:

```text
git clone
    ↓
Open in VS Code
    ↓
Reopen in DevContainer
    ↓
scripts/doctor
    ↓
scripts/check
    ↓
scripts/run
    ↓
open noVNC
    ↓
see Slint application
```

and a coding agent inside the same DevContainer can:

```text
inspect source
    ↓
edit Rust/Slint
    ↓
compile/check
    ↓
run tests
    ↓
launch application
    ↓
inspect semantic UI
    ↓
perform semantic actions
    ↓
inspect application state
    ↓
capture screenshot
    ↓
read structured logs
```

without requiring manual operations on the macOS host.

Before implementation, briefly inspect current Slint documentation for the selected version and verify the current Linux dependencies, compiler integration, accessibility APIs and recommended renderer/backend configuration. Do not rely on obsolete Slint examples.

Then implement the environment, run it, fix problems encountered, and leave the repository in a verified working state.