# Architecture

The repository follows a layered Rust architecture that isolates the business logic from the Slint desktop layer.

## Layer map
- `domain`: value objects, entities, rules, validation, errors.
- `application`: commands, queries, use cases, orchestration.
- `infrastructure`: filesystem/network/persistence adapters.
- `desktop`: Slint UI shell and callback translation.
- `agent-api`: stable semantic operations for agent tooling and future MCP adoption.

## UI boundary
The Slint UI layer does not implement business rules. It translates user action into application operations and binds to the authoritative Rust state. This keeps GUI verification focused on presentation and invocation semantics rather than behavioral logic duplication.

## Semantic IDs
Interactive controls must use stable IDs such as `main.input` and `main.submit`.
