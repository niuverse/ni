+++
id = "M9-008"
title = "Live web event tail"
status = "done"
milestone = "M9"
module = "web"
size = "M"
priority = "P0"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-007"]
docs = ["rendering", "adr-0036"]
+++

## Problem

The web viewer loaded one snapshot. A session running in another terminal never
appeared until the page was refreshed.

## Scope

- `GET /api/events` as a chunked `text/event-stream` tail of the JSONL log.
- Send the current lines first, then append new complete lines.
- Robust partial-line buffering; heartbeat comments while idle.
- Client uses `EventSource` and shows live/reconnecting state.
- No writes to the log; loopback only.

## Acceptance

- [x] SSE headers are chunked and no-store.
- [x] The initial session_start is delivered.
- [x] An appended assistant_message is delivered without refresh.
- [x] Partial lines wait for their newline.
- [x] The client has no remote assets and uses EventSource.
- [x] A loopback test covers initial + appended events.

## Notes

Still read-only: `/api/prompt` is 501. Browser chat and approvals need a
provider/agent loop inside the server, not a file tail.
