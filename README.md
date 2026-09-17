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
notes.title Test` followed by `scripts/agent click notes.save` to exercise the
Notes AppShell path. The legacy agent demo remains available through
`scripts/agent set main.input Test` and `scripts/agent click main.submit`.
Persistent settings, submissions, and notes are stored in a migrated SQLite
database under the platform data directory. The HTTP agent API is
development-only and listens on loopback.
MCP-capable coding agents can use `scripts/mcp` as a stdio server after
`scripts/run`; see `docs/agent-api.md` for its tools and resources.

For the complete agent testing loop:

```bash
scripts/run
scripts/mcp
# MCP client: ui_inspect -> app_command/ui_action -> app_state -> ui_screenshot
scripts/agent diagnostics
```

## CI and unsigned test release artifacts

Windows is the primary CI and release validation platform. The Windows workflow
runs formatting, production-posture checks, strict all-feature linting, the
Slint/agent API static contract checks, tests, `cargo-deny`, release EXE/MSI
builds, MSI install/uninstall validation, and an installed unsigned EXE smoke
test. macOS remains a secondary validation platform.

The "Test release artifacts" workflow can be run manually or by pushing a `v*`
tag. It intentionally produces unsigned validation artifacts only: Windows
EXE/MSI and macOS ZIP outputs are uploaded with SHA-256 checksums, CycloneDX
SBOMs, and GitHub build-provenance attestations. It does not perform code
signing or require signing certificates.

## Repository layout
See the architecture and docs folders for the intended production structure.

- `docs/design-system.md` — UI visual and interaction reference (Windows 11 /
  Fluent 2 design rules; enforceable constraints, not suggestions)
- `docs/development.md` — DevContainer workflow, auto-restart behavior, and
  troubleshooting
- `tests/ui/baselines/` — approved visual reference images
- `artifacts/failures/` — generated UI failure bundles (ignored by default)

## macOS validation builds

The macOS workflow is a secondary verification build for pull requests and
pushes to `master`. It produces the unsigned
`slint-demo-macos-arm64-unsigned` artifact for CI inspection. For `v*` tags,
it signs and notarizes a distribution only when its complete Apple signing
configuration is available; otherwise it emits the unsigned artifact and a CI
warning instead of failing the test release.

Das App-Bundle verwendet die Kennung `com.aheissenberger.slint-demo`.
Persistente Benutzerdaten werden dadurch passend zur Bundle-Identität unter
`~/Library/Application Support/com.aheissenberger.slint-demo` abgelegt.
Die gemeinsame Plist-Vorlage liegt unter `packaging/macos/Info.plist`.

### Enabling macOS signing and notarization

To enable the optional signed macOS artifact for a `v*` tag, configure all of
the following repository or environment secrets. The workflow deliberately
skips signing when any one of them, including `APPLE_ID`, is missing.

- `APPLE_CERTIFICATE_BASE64` — Base64-encoded `.p12` export containing the
  **Developer ID Application** certificate and its private key.
- `APPLE_CERTIFICATE_PASSWORD` — Password used to export that `.p12` file.
- `APPLE_DEVELOPER_ID_APPLICATION` — The complete Developer ID Application
  signing identity, for example `Developer ID Application: Example Company
  (ABCDE12345)`.
- `APPLE_ID` — Apple ID authorized for notarization.
- `APPLE_TEAM_ID` — Ten-character Apple Developer Team ID.
- `APPLE_APP_SPECIFIC_PASSWORD` — Apple-ID app-specific password for
  notarization; do not use the Apple-ID account password.

Use a protected GitHub Environment for these secrets and restrict it to
maintainers and protected version tags. The signing material is imported into
a temporary CI keychain and removed at the end of the signing job. The
notarized output is uploaded as `slint-demo-macos-arm64`; otherwise the
unsigned CI-inspection artifact remains `slint-demo-macos-arm64-unsigned`.
