+++
id = "M9-043"
title = "Start a new web session"
status = "done"
milestone = "M9"
module = "web"
size = "M"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-007"]
docs = ["rendering", "adr-0036"]
+++

## Problem

The web viewer was pinned to the session passed on the command line. `/clear`
only cleared the DOM; there was no honest way to start fresh.

## Scope

- Make the server's session path mutable under a mutex.
- `POST /api/new` creates a new JSONL in the current session's directory, appends
  `SessionStart` + `Policy`, and switches the server to it.
- Return 403 when `allow_new` is false, 409 while a turn runs.
- Update `/api/session`'s snapshot for the new file.
- `/new` in the browser posts, shows the id, and reloads; SSE reconnects to the
  new path.
- niuse sets `allow_new: false` because its thread/session semantics need an
  archive step first.

## Acceptance

- [x] `/api/new` creates another log and changes the active path.
- [x] The new file exists and has a session header.
- [x] Busy/disabled paths fail closed.
- [x] `/new` is in help and the command menu.
- [x] Existing path uses read a consistent locked clone.
- [x] Tests cover creation over loopback.

## Notes

Old SSE connections keep tailing the old file until the page reloads. A future
slice can broadcast a session-switched event and handle archived niuse threads.
