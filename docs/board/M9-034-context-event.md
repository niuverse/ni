+++
id = "M9-034"
title = "Context changed event over SSE"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-030"]
docs = ["rendering", "adr-0036"]
+++

## Problem

The sidebar refreshed on a 600ms timeout after `turn_end` plus a 15s poll. The
server knew exactly when a turn finished and the product context might have
changed, but never said so.

## Scope

- After a web turn finishes (success or error), broadcast
  `{"type":"context_changed"}` on the existing SSE channel.
- The client calls `loadContext()` immediately.
- Keep the interval and turn-end timeout as fallbacks.
- No new endpoint, store, or write path.

## Acceptance

- [x] The event is emitted after a scripted turn.
- [x] The SSE turn test waits for `context_changed`.
- [x] The browser reloads context on the event.
- [x] Existing context/timer/approval tests pass.
- [x] Static client assertions cover the event.

## Notes

This is a hint, not a snapshot; the client still fetches `/api/context`. External
edits to Markdown stores are covered by the periodic refresh.
