+++
id = "M9-009"
title = "Browser prompt, read-only"
status = "done"
milestone = "M9"
module = "web"
size = "M"
priority = "P0"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-008"]
docs = ["rendering", "adr-0036"]
+++

## Problem

The web viewer could tail a session but the browser could not start a turn.

## Scope

- `POST /api/prompt` accepts `{"prompt": "..."}` and returns 202.
- One turn at a time; a concurrent POST gets 409.
- Turns are hard-coded read-only, network off, approval never.
- The existing agent loop runs one prompt; the session log stays the truth.
- SSE shows the user/assistant events when written.
- Embedded composer posts the prompt.

## Acceptance

- [x] 202 on accepted prompt; 400 on empty; 409 while busy.
- [x] A scripted-provider test runs a turn and writes the answer to the log.
- [x] The browser has a prompt form and shows errors.
- [x] No approval or write path is exposed.
- [x] Server stays loopback-only and read-only by construction.

## Notes

No streaming deltas yet: the browser sees the assistant message when the turn
finishes. Approvals and delta streaming need an interactive runtime bridge, not
the callback engine used here.
