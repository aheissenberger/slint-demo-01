# Development

## Opening in VS Code
- Open this folder in VS Code.
- Reopen in the DevContainer.
- The container will start the Xvfb + x11vnc + noVNC stack.

## Apple Silicon architecture
The DevContainer is configured for `linux/arm64`, matching Apple Silicon Docker
Desktop hosts. Rebuild the DevContainer after pulling configuration changes so
Docker recreates the service with the ARM image. The environment is not
configured for Intel (`linux/amd64`) hosts.

## Accessing the GUI
- VNC: `localhost:5900`
- noVNC: `http://localhost:6080/vnc.html`

## Working inside the container
- `cargo check --workspace`
- `cargo test --workspace`
- `./scripts/check`
- `./scripts/test`
- `./scripts/run`
- `./scripts/screenshot`
- `./scripts/e2e` (after `./scripts/run`, semantic UI smoke test)

The DevContainer includes the pinned `actionlint` release used by
`./scripts/check` to validate GitHub Actions workflow files.
The development image also installs the pinned `cargo-watch` version used by
the desktop launcher; Windows packaging pins `cargo-wix` in CI for
reproducible installer builds. The supported macOS and Windows workflows also
run formatting, strict feature-complete linting, the complete test suite, and
repository static contract checks where appropriate. Windows is the primary
release target: its workflow additionally builds, validates, installs, launches
the installed unsigned EXE in headless installation-verification mode, and
uninstalls the MSI on a native Windows runner. The verification initializes the
runtime directory, process lock, SQLite database, and migrations. A visible GUI
launch is intentionally not used because GitHub-hosted Windows runners provide
no interactive desktop. Linux is used only for development tooling and
supply-chain auditing and is not a release build target.
Rust dependency policy lives in `deny.toml`; `cargo-deny` runs in the Windows
CI workflow and in the scheduled/manual supply-chain audit workflow.

The agent API contract is derived from the Slint source files. The
`slint-contract` helper compiles `ui/app.slint` with the official Slint
interpreter, then extracts `agent-id` declarations from the Slint files before
`scripts/agent-api-check` compares them with the Rust semantic API. This keeps
the `.slint` files authoritative without relying on a regex-only UI parser.
The end-to-end shell checks use structured `jq` assertions against the JSON
agent responses, rather than matching serialized field order with text tools.

The desktop process also starts the local agent API on `127.0.0.1:8080`.
The container starts Xvfb, Openbox, x11vnc, and websockify automatically;
noVNC is available at `http://localhost:6080/vnc.html`. Use
`./scripts/doctor --json` and `./scripts/agent diagnostics` for
machine-readable diagnostics. Docker and all compilation remain inside the
container; the host only needs Docker Desktop, VS Code, and Dev Containers.

## Desktop-Laufzeitdaten

Die Desktop-Anwendung verwendet das Betriebssystemdatenverzeichnis von
`com/aheissenberger/slint-demo`: unter Linux typischerweise
`~/.local/share/slint-demo`, unter macOS
`~/Library/Application Support/com.aheissenberger.slint-demo` und unter
Windows `%LOCALAPPDATA%\aheissenberger\slint-demo\data`. Darin liegen getrennt
von den fachlichen Anwendungsdaten die Laufzeitdateien unter `runtime/`:

Die macOS-Bundle-ID lautet ebenfalls `com.aheissenberger.slint-demo`. Dadurch
stimmen die Identität in `Info.plist` und der von macOS für private,
persistente Anwendungsdaten vorgesehene Ordner unter `Application Support`
überein. `SLINT_DEMO_DATA_DIR` kann den Datenordner weiterhin gezielt für
Tests und verwaltete Installationen überschreiben.

Unter Windows 10 und 11 wird der Pfad nicht aus einem fest codierten
Benutzerprofil zusammengesetzt, sondern über den Windows-Known-Folder
`FOLDERID_LocalAppData` ermittelt. Die lokale Datenbank, der Fensterzustand und
die Prozesssperre sind gerätebezogene Daten und liegen deshalb bewusst unter
`%LOCALAPPDATA%` statt im roamingfähigen `%APPDATA%`. Der MSI verwaltet nur
Programmdateien unter `%ProgramFiles%`; Updates und Deinstallation verändern
die persönlichen Anwendungsdaten nicht.

- `instance.lock` stellt sicher, dass nur eine Desktop-Instanz gleichzeitig
  läuft. Ein zweiter Start beendet sich ohne eine weitere Benutzeroberfläche.
- `window-state.json` speichert Größe und Position des Hauptfensters. Auf
  Wayland kann die Fensterverwaltung das Wiederherstellen der Position
  ablehnen; die Größe wird weiterhin wiederhergestellt.

Beim Schließen wird ein laufender Übermittlungsvorgang über denselben
kooperativen Abbruchpfad wie die Schaltfläche **Abbrechen** beendet. Erfolgreich
abgeschlossene oder fehlgeschlagene Übermittlungen erzeugen eine native
Systembenachrichtigung. Falls der Benachrichtigungsdienst des Betriebssystems
nicht verfügbar ist, wird der Fehler strukturiert protokolliert, ohne den
Anwendungsvorgang zu verändern.

The local API accepts bounded HTTP/1.1 requests only: headers are limited to
8 KiB, JSON request bodies to 8 KiB, and individual reads and writes time out
after five seconds. It returns explicit `400` or `413` JSON errors for invalid
or oversized requests.

The post-create hook installs the pinned Rust components, fetches locked
dependencies, and runs doctor, check, and unit/application tests. Runtime
semantic checks remain explicit: run `./scripts/e2e` after `./scripts/run`.

Beim Containerstart bereinigt
`.devcontainer/scripts/cleanup-cargo-target.sh` das persistente Cargo-Target
höchstens einmal innerhalb von 24 Stunden. Release-Artefakte werden entfernt,
weil der Container ausschließlich Entwicklungs-Builds ausführt. Incremental-
Sessions, die länger als `CARGO_TARGET_RETENTION_DAYS` nicht verändert wurden,
werden ebenfalls entfernt. Überschreitet das gesamte Target
`CARGO_TARGET_MAX_GIB`, wird der Inhalt des Build-Caches vollständig
zurückgesetzt, ohne den Docker-Volume-Mountpoint selbst zu entfernen. Die
Compose-Standardwerte sind 14 Tage und 40 GiB. Nach einer vollständigen
Bereinigung dauert der nächste Build entsprechend länger.

The Compose command starts the desktop through `./scripts/run --watch`. The
runner uses `cargo watch` to rebuild whenever a watched source file changes
(`crates/`, `ui/`, `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`). It keeps
the current desktop process alive while compiling and restarts it only after a
successful build has produced a new binary. If a build fails, the old app keeps
running in noVNC until the next successful build. Editing UI or Rust code and
saving is enough to see the change reflected; no manual restart or DevContainer
rebuild is required. Running `./scripts/run` again is safe: it detects the
existing application and prints the noVNC URL instead of starting a second
process on port 8080.

The Compose setup persists selected VS Code and Copilot state with Docker named
volumes so extension data and chat sessions survive DevContainer rebuilds. It
does not persist the entire `/root/.vscode-server` directory because that can
retain a partial server download or stale runtime state and prevent the remote
connection from starting after a rebuild. If VS Code reports that
`/root/.vscode-server/.../node` is missing, recreate the DevContainer so the VS
Code server binaries are installed afresh while the selected state volumes
remain intact.

The persisted VS Code paths follow the VS Code Dev Containers recommendation
for avoiding extension reinstalls: `/root/.vscode-server/extensions`,
`/root/.vscode-server/extensionsCache`, and
`/root/.vscode-server/data/User/globalStorage`. The project also persists the
agent chat directory at `/root/.vscode-server/data/agentSessionData` and the
minimal Copilot session content directory at `/root/.copilot/session-state`.
Other Copilot runtime files, logs, plugin installations, caches, and VS Code
server binaries remain ephemeral.

If a persistent volume itself becomes stale, remove only that named volume from
Docker Desktop or with `docker volume rm`; do not add a broad mount for
`/root/.vscode-server` as a workaround.

If noVNC displays “Failed to connect to server”, rebuild the DevContainer so
the updated x11vnc/websockify configuration is used, then reload
`http://localhost:6080/vnc.html`. The backend must expose an `RFB 003.008`
greeting on port 5900.

If noVNC connects but shows only a black screen, recreate or restart the
DevContainer so its long-running launcher picks up the current
`slint-demo` command. A stale launcher from an earlier crate name can keep
retrying a nonexistent binary while Xvfb and VNC remain healthy.

The development launch scripts enable the opt-in `agent-api` feature and start
the loopback-only development agent API. A default build omits that listener;
use `cargo build -p slint-demo --no-default-features` to make the
production-shaped posture explicit. The current container is a Linux
development runtime; Windows release builds should use native Windows CI
runners.
The Windows GitHub Actions workflow builds that binary on `windows-latest` and
uploads `slint-demo.exe` as the `slint-demo-windows-x86_64` workflow artifact.
It then packages the binary into an MSI installer using
[cargo-wix](https://github.com/volks73/cargo-wix) and the WiX Toolset, and
uploads it as the `slint-demo-windows-x86_64-installer` artifact. The German
installer places the app under `%ProgramFiles%\Slint Agent Desktop\bin`, adds a
Start Menu shortcut, and
supports upgrade/uninstall via the standard Windows "Apps & Features" list; it
targets Windows 10 and later. The WiX source lives in
`crates/slint-demo/wix/main.wxs` (generated with `cargo wix init` and then
hand-edited to add the Start Menu shortcut and German MSI metadata) and the
German MIT license text in `crates/slint-demo/wix/License.rtf` is maintained
as the installer's license dialog content. Regenerate the WiX template with
`cargo wix init --force -p slint-demo` if the license text changes; re-apply
the Start Menu shortcut edit, German language settings, German license text,
and the `$(sys.SOURCEFILEDIR)License.rtf` source paths afterward since
`init --force` overwrites `main.wxs`.
The Windows workflow runs on pushes to `master` and pull requests. The macOS
workflow is a secondary verification build for those events and creates
unsigned inspection artifacts. For `v*` tags it signs and notarizes only when
the complete Apple signing configuration, including `APPLE_ID`, is available;
otherwise it reports a warning and retains the unsigned artifact.

To enable signed and notarized macOS artifacts, configure these GitHub
repository or protected-environment secrets: `APPLE_CERTIFICATE_BASE64`
(Base64 `.p12` Developer ID Application certificate including private key),
`APPLE_CERTIFICATE_PASSWORD`, `APPLE_DEVELOPER_ID_APPLICATION`, `APPLE_ID`,
`APPLE_TEAM_ID`, and `APPLE_APP_SPECIFIC_PASSWORD`. See
[README.md](../README.md#enabling-macos-signing-and-notarization) for the
required formats and secret-handling guidance.
The separate "Test release artifacts" workflow can be run manually or by
pushing a `v*` tag to produce unsigned test artifacts without certificate or
code-signing requirements. It uploads Windows EXE/MSI and unsigned macOS ZIP
artifacts with SHA-256 checksums, CycloneDX SBOMs, and GitHub build-provenance
attestations. These artifacts are for release validation; they are not signed
end-user installers.
The Rust toolchain is pinned in `rust-toolchain.toml`; update that file and the
matching DevContainer installation command together when upgrading Rust.
