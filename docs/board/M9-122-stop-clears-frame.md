+++
id = "M9-122"
title = "Emergency stop clears the cached approval frame"
status = "done"
milestone = "M9"
module = "tools"
size = "S"
priority = "P1"
updated = "2026-10-24"
completed = "2026-10-24"
depends_on = ["M9-116", "M9-101"]
docs = ["adr-0037"]
+++

## Problem

The latest observation thumbnail was cached in memory for approvals. After an
emergency stop, it could remain reachable until the next observation.

## Scope

- Clear `ComputerRuntime.last_frame` inside `emergency_stop`, before backend
  release.
- Cover observe → frame available → stop → frame gone.
- Applies to TUI/REPL/exec/web stop, all of which call emergency_stop.
- No change to normal observe or approval thumbnails.

## Acceptance

- [x] Frame cache is populated after observe.
- [x] Emergency stop clears it.
- [x] Policy still turns off and runtime disarms.
- [x] Tests/clippy pass.

## Notes

This is memory hygiene, not durable-file cleanup; kept frames require explicit
store cleanup.
