+++
id = "M9-027"
title = "TUI tool markers"
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

In the TUI a tool call only changed the footer, and only failures reached the
transcript. A finished turn had no compact record of which tool types ran.

## Scope

- Print one `→ name` marker per tool type per turn.
- Do not print paths, commands, inputs, or successful outputs; those stay in
  details and the completion summary.
- Keep failures explicit as `✗ name · summary`.
- Reset the marker set on `TurnEnd`.
- Clip to viewport width; color arrows accent and failures danger.

## Acceptance

- [x] Each tool type appears at most once per turn.
- [x] No file paths from a 100-write bulk turn reach the transcript.
- [x] Failures remain visible.
- [x] Width is clipped.
- [x] Volume/PTY terminal tests pass.
- [x] Unit tests cover call/result formatting.

## Notes

This preserves the bounded transcript design: success evidence stays merged in
the completion line. The web surface owns full tool cards (M9-025).
