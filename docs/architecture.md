# Architecture

The repository follows a layered Rust architecture that isolates the business logic from the Slint desktop layer.

## Layer map
- `domain`: value objects, entities, rules, validation, errors.
- `application`: commands, queries, use cases, orchestration.
- `infrastructure`: filesystem/network/persistence adapters.
- `slint-demo`: Slint UI shell and callback translation.
- `agent-api`: stable semantic operations for agent tooling and future MCP adoption.

## Persistence

`infrastructure::SqliteRepository` is the production repository returned by
`initialize_repository()`. It stores settings, submission records, and notes in
`app-data.sqlite3` below the platform data directory (overridable via the
`SLINT_DEMO_DATA_DIR` environment variable).

- Schema changes are applied through the `rusqlite_migration` crate. The
  repository opens the database, enables foreign keys, and migrates to the
  latest schema before application state is loaded.
- Version 1 creates the singleton settings row, submission records, notes, and
  an index for active notes ordered by update time.
- `infrastructure::FileRepository` remains available as a legacy JSON adapter
  covered by tests, but it is no longer the production default.
- `infrastructure::MemoryRepository` remains available as a pure in-memory
  adapter for tests; `SqliteRepository::in_memory()` offers the migrated SQLite
  behavior behind the production adapter's type for callers that need a
  drop-in, disk-free instance.

Both adapters implement `application::AppRepository` (settings and
submissions) and `application::NoteRepository` (the `Note` entity/use
cases in `application::NoteService`), which is a small, meaningfully-named
domain workflow — create/rename/archive/unarchive/delete — kept separate
from the generic UI submission demo flow.

## AppShell and notes workflow

The main Slint surface is now a desktop AppShell: a notes sidebar exposes the
active local notes, and the primary content pane edits, saves, archives, and
deletes the selected note. The former submission demo remains as a secondary
agent-testing section so existing semantic test paths continue to exercise the
shared state/update pipeline.

## Error, validation, and task flows

The application layer now separates user-facing recovery text from developer
diagnostics. `ApplicationError` exposes a stable `ErrorCode`, severity,
recoverability flag, localized user message, and diagnostic message. The live
`AppState` carries the currently visible `UiError`, field-level
`FieldValidation` entries, and the active background task list. Slint renders
recoverable problems as an inline alert with retry/dismiss actions; critical
data errors are surfaced through a modal alert dialog.

Inline validation is non-blocking: field edits update validation state without
running persistence, while disabled buttons and command errors still enforce
the same domain rules. Repository and migration failures are classified
separately from invalid input. Corrupt or incompatible SQLite data maps to a
recovery-required error so the UI and logs can guide the user without exposing
raw adapter diagnostics as product text.

Long-running submission work is represented by explicit task IDs. The store
tracks multiple concurrently running tasks, per-task progress, cancellation
tokens, retryability, and the last failed submission payload. `Cancel` targets
the active task, `Retry` restarts the last retryable failed submission, and
shutdown requests cancellation for the active operation before the window is
hidden. Adapter work checks the cancellation token cooperatively at application
boundaries before expensive or persistent operations are started.

## Settings persistence

`AppSettings` (domain) now carries a `theme_mode` field. `AppStateStore`
loads it at startup (falling back to `system` on any error) and persists it
whenever `AppAction::SetThemeMode` is dispatched, so the chosen appearance
survives an application restart. See `docs/agent-api.md` for the
agent-facing implications.

## UI boundary
The Slint UI layer does not implement business rules. It translates user action into application operations and binds to the authoritative Rust state. This keeps GUI verification focused on presentation and invocation semantics rather than behavioral logic duplication.

## Localization

Static Slint UI text uses Slint's built-in `@tr(...)` translation helper.
Translations are bundled at build time from
`crates/slint-demo/translations/<locale>/LC_MESSAGES/slint-demo.po` through
`CompilerConfiguration::with_bundled_translations`. German is the default
source language; additional locales can be added without changing domain
types or UI wiring. `application::Catalog` remains the boundary for dynamic
Rust-side state labels and errors.

## Semantic IDs
Interactive controls must use stable IDs such as `main.input` and `main.submit`.
