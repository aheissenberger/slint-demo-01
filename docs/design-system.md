# UI design system

This document is the reference for the Slint UI's visual and interaction
decisions. It applies to Windows production builds and to the Linux
DevContainer rendering used for development and automated verification.

## Design direction

Use a Windows 11 / Fluent-inspired desktop language without reproducing
Win32 controls or depending on Windows-only APIs. The design rules below are
implementation constraints for agents, not optional styling suggestions.
Prioritize decisions in this order:

1. usability
2. accessibility
3. familiar desktop interaction
4. consistency
5. clear hierarchy
6. responsive resizing
7. visual polish

Prefer clean surfaces, restrained color, rounded corners, clear typography,
light borders, subtle elevation, visible focus, and obvious interaction
states. Avoid gradients, glossy or skeuomorphic treatments, excessive cards,
heavy shadows, unnecessary separators, and oversized mobile-style controls.

Agents must not introduce arbitrary colors, radii, spacing, font sizes, or
control geometry in screen files. Add or update a token in `ui/theme/` first,
then consume that token from a reusable component. Do not call a generic
rectangle a card unless it groups related content and has a documented
hierarchy purpose.

## Tokens and components

When the UI grows beyond the current single-screen foundation, keep reusable
tokens under `ui/theme/` and reusable controls under `ui/components/`.
Components should consume tokens rather than scattering literal values across
`.slint` files.

The token set must cover:

- spacing, corner radii, control heights, icon sizes, and border widths
- semantic typography (`caption`, `body`, `body-strong`, `subtitle`, `title`)
- surface, text, accent, disabled, focus, error, warning, and success colors
- elevation and animation durations

Use a 4 px spacing rhythm where practical. Prefer the following initial scale:
`4`, `8`, `12`, `16`, `20`, `24`, and `32` px.

Use `Segoe UI Variable`, then `Segoe UI`, then a platform-appropriate fallback.
Do not bundle Microsoft fonts.

Use `Segoe UI` as the Slint fallback token when `Segoe UI Variable` cannot be
selected by the platform. Every visible `Text` and `TextInput` must consume the
font token rather than relying on the renderer default.

### Surface and geometry rules

- Use a page background plus one primary content surface; do not nest multiple
  opaque cards for a simple task.
- Corner radii follow Fluent 2 tokens, not arbitrary values:
  - `4 px` (`ControlCornerRadius`) for controls: buttons, text fields, small
    inputs.
  - `8 px` (`OverlayCornerRadius`) for cards, dialogs, flyouts, and the main
    content surface.
  - Never give a control a larger radius than its containing surface.
- Keep standard controls 32 px high and text fields 36 px high, excluding an
  external label. A labelled field must have enough height for the label,
  spacing, and field without clipping.
- Prefer a 1 px neutral border. Use a 2 px border only for invalid or focused
  states where the extra contrast is meaningful.
- Do not stretch short forms vertically. Layouts must opt into start alignment
  or use explicit content heights so empty space does not become accidental
  hierarchy.
- Give the primary content surface real elevation: apply the `shadow` token as
  a drop shadow (small blur, small positive y-offset) rather than leaving it
  unused. Do not add shadows to individual small controls such as buttons;
  Fluent 2 keeps those flat and relies on background/border state changes
  instead.

### Interaction and state rules

- Every control must expose a stable semantic ID, accessible role, and label.
- Focus must remain visible without relying on color alone.
- Hover and pressed states must be distinct but restrained; do not use
  gradients or bevels.
- Disabled controls must be visibly disabled and must not trigger callbacks.
- Primary actions should be visually prominent but must not use fully saturated
  legacy blue by default; consume the accent tokens.

### Window and platform rules

Use the native window frame on Windows so the OS supplies the title-bar buttons,
snap behavior, and accessibility integration. Always set a meaningful window
title. Linux development screenshots may show the host window manager's
decorations and are not evidence of native Windows title-bar fidelity. Agents
must not imitate title-bar buttons inside the content area unless a real custom
window-chrome implementation is explicitly required and tested on every target.

### Theme rules

Keep light and dark semantic roles paired when adding theme support. Never
hard-code a light-only surface into a component. System accent integration and
high-contrast behavior belong in the theme/platform adapter, not in screen
layout code.

## Layout and interaction

Choose the simplest structure that supports the task. A single-purpose screen
should not gain navigation or a dashboard without a user need. Multi-section
applications may use left navigation with an unambiguous current selection.

Every interactive control must have:

- a stable semantic identifier
- an accessible role and label
- a visible keyboard-focus state
- a keyboard path that does not depend on pointer coordinates

Do not use translated display text, color alone, placeholder text alone, or
screen coordinates as the only means of identifying or operating a control.
Keep business logic in Rust; Slint should translate user actions into
application operations.

Before adding a screen, define its primary task and action, required
immediate information, empty/loading/error states, semantic IDs, keyboard
path, and behavior at 1024x768 and with longer translated strings.

## Verification

UI changes should be checked with the repository's normal validation commands,
then reviewed through the semantic inspection/action surface when available
and with a captured screenshot. Linux/Xvfb screenshots are useful regression
evidence but are not proof of exact Windows rendering; native Windows
validation remains authoritative for that target.

Screenshot baselines belong in `tests/ui/baselines/`. Do not update an
approved baseline silently when a visual test fails.

When a UI test fails, collect the failure bundle under
`artifacts/failures/<test-name>/`:

```text
state.json       application state
ui.json          semantic UI tree
screenshot.png   rendered pixels
logs.jsonl       structured diagnostics
environment.json runner and platform details (when available)
```

These artifacts are complementary: they describe application state,
semantics, pixels, and diagnostics without requiring interactive VNC
debugging. Failure output is generated evidence and must not be committed
unless a particular investigation explicitly requires it.
