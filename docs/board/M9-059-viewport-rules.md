+++
id = "M9-059"
title = "Viewport-width horizontal rules in the TUI"
status = "done"
milestone = "M9"
module = "tui"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-018"]
docs = ["rendering", "adr-0035"]
+++

## Problem

Horizontal rules were a fixed 40 cells. On a wide terminal they looked like a
stray fragment; on a narrow one they could wrap.

## Scope

- Detect a line of three or more `─` cells with optional leading spaces.
- Expand it to `viewport - 1 - indent` (minimum 3) at emission time.
- Preserve indentation; leave equal-width lines untouched.
- Apply the muted rule tone now that the expanded glyph is recognized.
- No parser or event-log change.

## Acceptance

- [x] Rules expand to width−1 with indentation preserved.
- [x] Non-rule lines are untouched.
- [x] Equal-width rules are not reallocated.
- [x] PTY rule assertion still passes.
- [x] Inline and linear paths share the emitted line.

## Notes

The expansion happens at the terminal boundary, so stored Markdown and the event
log keep the original compact rule.
