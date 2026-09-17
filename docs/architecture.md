# Architecture

The repository follows a layered Rust architecture that isolates the business logic from the Slint desktop layer.

## Layer map
- `domain`: value objects, entities, rules, validation, errors.
- `application`: commands, queries, use cases, orchestration.
- `infrastructure`: filesystem/network/persistence adapters.
- `slint-demo`: Slint UI shell and callback translation.
- `agent-api`: stable semantic operations for agent tooling and future MCP adoption.

## Persistence

`infrastructure::FileRepository` is the production repository returned by
`initialize_repository()`. It stores settings, submission records, and notes
as a single versioned JSON document (default location: the platform data
directory, overridable via the `SLINT_DEMO_DATA_DIR` environment variable).

- Reads are resilient: a missing file starts with defaults, and a corrupt
  file is backed up (`*.corrupt-<unix-seconds>.bak`) next to itself and
  replaced with defaults instead of crashing the application.
- Writes are atomic (temp file + rename) so an interrupted write cannot
  leave a truncated, unparsable file behind.
- Older on-disk data migrates forward automatically: new optional fields
  deserialize via `#[serde(default)]`, and the repository's `migrate` step
  upgrades the stored schema `version` on next save.
- `infrastructure::MemoryRepository` remains available as a pure in-memory
  adapter for tests; `FileRepository::in_memory()` offers the same
  behavior behind the persistent adapter's type for callers that need a
  drop-in, disk-free instance.

Both adapters implement `application::AppRepository` (settings and
submissions) and `application::NoteRepository` (the `Note` entity/use
cases in `application::NoteService`), which is a small, meaningfully-named
domain workflow — create/rename/archive/unarchive/delete — kept separate
from the generic UI submission demo flow.

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
