+++
id = "M9-058"
title = "Reduced-motion fallback in the TUI"
status = "done"
milestone = "M9"
module = "tui"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-006"]
docs = ["rendering", "adr-0035"]
+++

## Problem

The web shell honors `prefers-reduced-motion`, but the TUI spinner/caret had no
explicit opt-out beyond disabling color or using a non-TTY.

## Scope

- `NIUCODE_REDUCED_MOTION=1` or `NO_MOTION=1` disables the animated braille
  frame and blinking caret.
- Status text, elapsed time, and tool count remain.
- The static fallback marker remains.
- No CLI flag or event-log change.

## Acceptance

- [x] Animation is disabled when either environment variable is present.
- [x] Static caret/frame fallback is used.
- [x] Status content stays the same.
- [x] Existing activity tests pass.
- [x] Documented in the rendering roadmap.

## Notes

This is presence-based, not value-parsed, matching `NO_COLOR` conventions. A
CLI flag can be added if automation needs it per invocation.
