# Slint UI requirements

These requirements apply to every `.slint` file below `ui/`. They are mandatory
for Windows-targeted UI work. Read [the design system](../docs/design-system.md)
before making a visual or interaction change.

## Fluent 2 implementation rules

- Build a Windows 11 Fluent 2 application surface, not a generic web card or a
  loose "Fluent-inspired" design.
- Use only semantic values from `theme/theme.slint` for colors, spacing,
  typography, corner radii, control sizes, borders, shadows, and motion. Add a
  named token before a value is needed; do not place a literal visual value in
  a screen or component.
- Compose screens from reusable controls under `components/`. A screen may
  arrange those controls but must not recreate button, text-field, card, focus,
  hover, pressed, disabled, error, or navigation treatments inline.
- Use Fluent 2 geometry: 4 px control corners, 8 px overlay/surface corners,
  a 4 px spacing rhythm, compact desktop control heights, and Segoe UI
  Variable/Segoe UI typography through the theme token.
- Prefer a clear application hierarchy (app/page header, contextual command
  area, and content) over a large centered card. Use cards only to group a
  distinct task or information set; do not use a card merely as a page
  background.
- Keep controls flat with restrained state changes. Do not introduce gradients,
  glass effects, bevels, excessive rounding, mobile-sized controls, or
  prominent shadows. Elevation is reserved for a primary surface, dialog,
  flyout, or other layered content.
- Preserve native Windows window chrome. Do not draw imitation caption buttons
  or title bars in Slint unless custom chrome is explicitly required and
  validated on every supported target.

## Required interaction and accessibility

- Every interactive control requires a stable `agent-id`, accessible role,
  accessible label, keyboard access, visible focus, and distinct hover,
  pressed, disabled, and invalid states where applicable.
- Never communicate state solely through color, placeholder text, pointer
  position, or translated display text.
- Keep business rules and platform integration in Rust. Slint only presents
  state and forwards semantic actions.

## Required review

Before completing a `.slint` change, run the UI checks required by the root
`AGENTS.md`, inspect the semantic tree, and capture a screenshot. Review it
using the checklist in `docs/design-system.md`. Linux/Xvfb rendering validates
regressions only; Windows rendering is authoritative for Windows fidelity.
