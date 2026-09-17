# Slint Agent Desktop

This repository establishes a devcontainer-first Rust + Slint desktop application foundation for agentic software development.

## Host requirements
- macOS
- Docker Desktop
- VS Code
- Dev Containers extension

## Container requirements
The full environment runs inside a Linux DevContainer and includes:
- Rust stable toolchain
- cargo, rustfmt, clippy, rust-analyzer
- Slint + compiler support
- Xvfb virtual display
- Openbox window manager
- x11vnc and noVNC
- VS Code extension recommendations

## Quick start
1. Open this folder in VS Code.
2. Reopen in Container using Dev Containers.
3. Run `./scripts/doctor`, then `./scripts/check` and `./scripts/test` from inside the container.
4. Run `./scripts/run` from inside the container.
5. Open the forwarded port `6080` at `http://localhost:6080/vnc.html`.

The container starts Xvfb, Openbox, x11vnc, and websockify automatically. The
desktop app runs under `cargo watch`, so editing Rust or Slint source
(`crates/`, `ui/`) automatically recompiles and restarts the running app in
noVNC after a successful build. If a build fails, the previous desktop process
keeps running; the runner restarts it only after a new binary exists. No manual
restart or DevContainer rebuild is needed.
Use `scripts/agent ui` to inspect semantic controls, `scripts/agent set
main.input Test` followed by `scripts/agent click main.submit` to exercise the
UI path, or `scripts/agent command submit Test` to exercise application intent
directly. The HTTP agent API is development-only and listens on loopback.
MCP-capable coding agents can use `scripts/mcp` as a stdio server after
`scripts/run`; see `docs/agent-api.md` for its tools and resources.

For the complete agent testing loop:

```bash
scripts/run
scripts/mcp
# MCP client: ui_inspect -> app_command/ui_action -> app_state -> ui_screenshot
scripts/agent diagnostics
```

## Repository layout
See the architecture and docs folders for the intended production structure.

- `docs/design-system.md` — UI visual and interaction reference (Windows 11 /
  Fluent 2 design rules; enforceable constraints, not suggestions)
- `docs/development.md` — DevContainer workflow, auto-restart behavior, and
  troubleshooting
- `tests/ui/baselines/` — approved visual reference images
- `artifacts/failures/` — generated UI failure bundles (ignored by default)

## macOS distribution builds

The macOS workflow produces a signed, notarized, and stapled application for
pushes to `main` and manual runs. These steps are required for an application
downloaded from GitHub to pass Gatekeeper; an unsigned application is not a
release artifact.

Configure these GitHub Actions secrets before running a distribution build:

- `APPLE_CERTIFICATE_BASE64` — Base64-encoded `.p12` export of the **Developer
  ID Application** certificate and private key.
- `APPLE_CERTIFICATE_PASSWORD` — Password used when exporting that `.p12`.
- `APPLE_DEVELOPER_ID_APPLICATION` — Signing identity, for example `Developer
  ID Application: Example Company (ABCDE12345)`.
- `APPLE_ID` — Apple ID used for notarization.
- `APPLE_TEAM_ID` — Apple Developer Team ID.
- `APPLE_APP_SPECIFIC_PASSWORD` — App-specific password for that Apple ID.

Pull-request builds intentionally publish an artifact named
`slint-demo-macos-arm64-unsigned`; use it only for CI inspection, not end-user
distribution. Download `slint-demo-macos-arm64.zip` from a successful
distribution build, extract it with Finder, and open the resulting
`slint-demo.app`.
