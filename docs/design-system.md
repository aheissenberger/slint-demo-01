# UI design system

This document is the reference for the Slint UI's visual and interaction
decisions. It applies to Windows production builds and to the Linux
DevContainer rendering used for development and automated verification.

## Design direction

Use the Windows 11 Fluent 2 design language. "Fluent-inspired" is not an
acceptable target: the rules below are implementation constraints for agents,
not optional styling suggestions. Native Windows behavior may be supplied by
the platform, but the Slint content must follow Fluent 2 hierarchy, token, and
interaction conventions without reproducing Win32 controls or depending on
Windows-only APIs.
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

The scoped requirements in `ui/AGENTS.md` are mandatory for every `.slint`
change. They exist to prevent a superficially modern but generic "web card"
appearance from replacing Fluent 2 desktop hierarchy.

### Current baseline: why the app does not yet read as Windows 11

The current screen is a useful functional foundation, but its visual language
is still only partially Fluent 2:

- `MainWindow` is a flat header followed by an unlayered form. There is no
  deliberate primary content surface or contextual command hierarchy, so the
  result reads like a simple web form rather than a Windows desktop page.
- The header uses a separator but does not establish a Fluent title/page
  hierarchy. A header rectangle, a blue button, and a light background are not
  sufficient evidence of Windows 11 design.
- `Theme.shadow` is defined but the screen does not apply it to a meaningful
  surface. Conversely, adding decorative shadows to controls would be the
  wrong correction: Fluent controls remain flat.
- The theme exposes `Segoe UI Variable`; when it is unavailable, Slint uses the
  platform font resolver's fallback. Linux screenshots can therefore differ
  from Windows typography without proving a Windows fidelity problem.
- Shared controls have the right starting geometry, but every state must be
  reviewed as a complete interaction system: keyboard focus, hover, pressed,
  disabled, and invalid states must be visible and semantically exposed. An
  opacity change or a border color alone is not an adequate state design.
- A reusable component can still break the token contract: for example,
  literal borders, dimensions, colors, or typography in a component bypass
  the theme and make later Fluent tuning inconsistent.

This diagnosis is a review aid, not permission to redesign the app into a
dashboard or to add cards without a task-based hierarchy. Fix composition and
tokens in the smallest coherent increment, then validate the result against
the checklist below.

### What makes a screen recognizably Windows 11

A Fluent 2 token palette alone does not make an application look like Windows
11. The composition must also provide:

- a deliberate application/page hierarchy: title or header, contextual
  commands when needed, then content;
- compact desktop density, clear alignment, and whitespace that separates
  sections rather than framing one oversized centered card;
- layered surfaces only where hierarchy calls for them (for example, dialogs,
  flyouts, or a distinct task group), with subtle elevation;
- Segoe UI Variable (or Segoe UI fallback) and Fluent typography roles;
- standard Fluent 2 control geometry and complete, restrained control states;
- native Windows caption buttons, snapping, and system accessibility rather
  than simulated title-bar UI in content.

Do not treat the following as Fluent 2 evidence: a white rounded rectangle, a
blue button, a drop shadow, or Linux/Xvfb window decorations. Each may be
appropriate, but none establishes Windows 11 visual fidelity on its own.

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
- Buttons, text fields, and other form controls must never be wider than the
  inner content width of their containing surface. When a container has visual
  insets, set the child layout's `x`, `y`, `width`, and `height` to the
  inset-adjusted content box before assigning controls `width: parent.width`;
  do not rely on layout padding if that makes children measure against the
  outer container width.
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

Slint rendering is not itself a Windows controls implementation. When exact
native behavior, materials, system accent response, high contrast, or caption
button behavior is required, validate it on supported Windows hardware and
record any accepted platform limitation in the change description.

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

### Mandatory Fluent 2 review checklist

For every visual `.slint` change, review the rendered screen before declaring
it complete:

1. **Hierarchy:** Does the screen read as an application page with a clear
   title, context, and primary action, rather than a centered website card?
2. **Tokens:** Are every color, size, radius, border, shadow, font, and state
   value supplied by the theme/component system?
3. **Controls:** Do buttons and fields use the shared Fluent components with
   compact geometry and distinct hover, pressed, focus, disabled, and invalid
   states as applicable?
4. **Surface use:** Are cards and elevation reserved for meaningful grouping or
   layering, with no nested opaque surfaces or decorative shadows?
5. **Accessibility:** Do controls retain stable semantic IDs, accessible roles
   and labels, visible focus, and keyboard operation?
6. **Platform fidelity:** If chrome or platform appearance changed, has the
   result been validated on Windows? Linux/Xvfb screenshots cannot approve
   Windows-specific fidelity.

If any answer is no, revise the design or document a deliberate,
tested exception before merging.
