+++
id = "M9-097"
title = "X11 observation-bound input"
status = "done"
milestone = "M9"
module = "tools"
size = "M"
priority = "P0"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-096"]
docs = ["adr-0037"]
+++

## Problem

The X11 backend could observe but not click, type, press keys, or scroll; the
protocol's observation-bound actions had no executor.

## Scope

- XTEST pointer motion, button press/release, key press/release, scroll buttons.
- Validate the observation token/expiry and live window geometry before any X
  request.
- Map image-relative points into root coordinates.
- Set input focus before type/key.
- ASCII text plus newline/tab/space and named keys (Return/Tab/Escape/arrows/
  F1–F12/modifiers).
- Reject unsupported Unicode and unknown keys instead of guessing.
- Xvfb integration: click and typed key events reach a test window.

## Acceptance

- [x] Stale/expired observations are rejected before XTEST.
- [x] Moved windows are rejected.
- [x] Click reaches the observed window.
- [x] Typed key reaches the focused window.
- [x] Unsupported text/key errors are explicit.
- [x] Clippy/tests pass.

## Notes

XTEST cannot type arbitrary Unicode without a keymap; this slice is ASCII-first.
Clipboard/paste and non-ASCII input need a separate design. X11 still has no
application isolation.
