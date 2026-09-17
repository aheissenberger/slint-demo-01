# Testing strategy

This project is designed for deterministic verification in the Linux DevContainer.

## Unit tests
The domain and application layers have focused tests for rules, validation, and command execution.

Run `./scripts/test` for the normal suite. Semantic API coverage includes the
empty-input/disabled-submit and set-value/submit transitions. GUI smoke tests
should remain focused and run separately under the virtual display.
Run `./scripts/e2e` after `./scripts/run` for the deterministic semantic smoke
workflow; it verifies the disabled/enabled submit transition without relying on
screen coordinates or pixels.

## Integration tests
The Rust integration suites live in `crates/agent-api/tests/` and cover the
semantic contract, command validation, and MCP stdio protocol. The Rust semantic contract test lives in `crates/agent-api/tests/semantic_contract.rs`
and runs as part of `./scripts/test`. The runtime checks in
`tests/integration/semantic_agent.sh` and `tests/ui/semantic_smoke.sh` are run by
`./scripts/e2e` after the desktop and agent API are available.

## UI smoke tests
The `tests/ui` checks semantic UI validation through the app inspection and
action API, confirms that the real `desktop` window exists on X11, and captures
a rendered screenshot. They are deliberately separate from the normal test
suite so unit and application tests never require a graphical session.

Approved visual reference images belong in `tests/ui/baselines/`. Keep the
baseline name tied to the test or screen it represents, and never replace a
baseline automatically as part of a failing test.

## Screenshot evidence
Screenshots are stored under `artifacts/screenshots/` for agent review and regression visibility.

## Failure artifacts
Store generated diagnostics for a failed UI test under
`artifacts/failures/<test-name>/`. When the relevant tooling is available, a
failure bundle should contain:

```text
state.json
ui.json
screenshot.png
logs.json
environment.json
```

The files provide independent views of application state, semantic structure,
rendered pixels, diagnostics, and execution context. Failure artifacts are
generated evidence and should not be committed by default.
