+++
id = "M9-051"
title = "Per-tool timing history"
status = "done"
milestone = "M9"
module = "tui"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-006", "M9-027"]
docs = ["rendering", "adr-0035"]
+++

## Problem

The footer showed only the currently running tool and the whole-turn elapsed
time. After completion there was no record of which tool consumed the time.

## Scope

- Track elapsed time per tool name while a tool runs.
- Aggregate per name (not per call), so a 100-write turn stays one line.
- At task completion, print `tools · name 1.2s · ...`, sorted by duration,
  bounded to six names and clipped to width.
- Clear after the task summary.
- No transcript writes per call and no event-log change.

## Acceptance

- [x] Durations aggregate by tool name.
- [x] Output is deterministic, sorted, and width-clipped.
- [x] Empty map produces no line.
- [x] Volume/PTY terminal tests still pass.
- [x] No file paths or outputs enter the timing line.

## Notes

Timing is presentation-only and lives in memory. Durable per-call timing would
need a new event field and is out of scope.
