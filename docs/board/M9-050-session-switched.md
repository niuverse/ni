+++
id = "M9-050"
title = "Broadcast session switches over SSE"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-049"]
docs = ["rendering", "adr-0036"]
+++

## Problem

Only the tab that posted `/api/switch` or `/api/new` reloaded. Other open tabs
kept tailing the old session and showed stale state.

## Scope

- After a successful switch/new, broadcast `{"type":"session_switched"}` on the
  existing live channel.
- Client reloads on the event; the requester's timed reload remains harmless.
- No new endpoint or storage.
- Add an SSE assertion to the switch test.

## Acceptance

- [x] Switch broadcasts `session_switched`.
- [x] New session broadcasts it too.
- [x] The switch HTTP test reads the event over SSE.
- [x] Client reloads on the event.
- [x] Existing prompt/approval/context tests pass.

## Notes

This closes the cross-tab window; connections still finish against the old file
until the reload is processed.
