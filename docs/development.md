# Development

## Opening in VS Code
- Open this folder in VS Code.
- Reopen in the DevContainer.
- The container will start the Xvfb + x11vnc + noVNC stack.

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

The desktop process also starts the local agent API on `127.0.0.1:8080`.
The container starts Xvfb, Openbox, x11vnc, and websockify automatically;
noVNC is available at `http://localhost:6080/vnc.html`. Use
`./scripts/doctor --json` and `./scripts/agent diagnostics` for
machine-readable diagnostics. Docker and all compilation remain inside the
container; the host only needs Docker Desktop, VS Code, and Dev Containers.

The post-create hook installs the pinned Rust components, fetches locked
dependencies, and runs doctor, check, and unit/application tests. Runtime
semantic checks remain explicit: run `./scripts/e2e` after `./scripts/run`.

The Compose command starts the desktop automatically under `cargo watch`,
which recompiles and restarts the running app whenever a watched source file
changes (`crates/`, `ui/`, `Cargo.toml`, `Cargo.lock`,
`rust-toolchain.toml`). Editing UI or Rust code and saving is enough to see
the change reflected in noVNC; no manual restart or DevContainer rebuild is
required. Running `./scripts/run` again is safe: it detects the existing
application and prints the noVNC URL instead of starting a second process on
port 8080.

If noVNC displays “Failed to connect to server”, rebuild the DevContainer so
the updated x11vnc/websockify configuration is used, then reload
`http://localhost:6080/vnc.html`. The backend must expose an `RFB 003.008`
greeting on port 5900.

The default desktop feature starts the loopback-only development agent API.
Use `cargo build -p desktop --no-default-features` for a production-shaped
binary without that listener. The current container is a Linux development
runtime; Windows release builds should use native Windows CI runners.
The Rust toolchain is pinned in `rust-toolchain.toml`; update that file and the
matching DevContainer installation command together when upgrading Rust.
