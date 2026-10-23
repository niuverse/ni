+++
id = "M9-084"
title = "PTY coverage for task and alert Markdown"
status = "done"
milestone = "M9"
module = "tui"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-081"]
docs = ["adr-0035"]
+++

## Problem

The PTY regression covered headings, emphasis, tables, code, math, lists, and a
plain quote, but not GFM alert labels or the absence of raw alert markers.

## Scope

- Add `> [!WARNING] careful` to the replay fixture.
- Assert the rendered `│ warning · careful` line.
- Assert `[!WARNING]` never leaks to the terminal.
- Keep task-list coverage.
- No renderer change.

## Acceptance

- [x] PTY test passes with the warning quote.
- [x] Alert label renders.
- [x] Raw marker is absent.
- [x] Existing heading/table/code/math assertions pass.
- [x] No fixed-width rule assumption in PTY.

## Notes

The rule-width expansion remains covered by the terminal unit test; the PTY
fixture deliberately avoids coupling to viewport width.
