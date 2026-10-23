+++
id = "M9-010"
title = "Stream web turns as they happen"
status = "done"
milestone = "M9"
module = "web"
size = "M"
priority = "P0"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-009"]
docs = ["rendering", "adr-0036"]
+++

## Problem

`POST /api/prompt` wrote the session at turn end; the browser saw the whole
answer at once. That is not comparable to a modern agent surface.

## Scope

- Bridge `AgentEvent::TextDelta` (and other runtime events) to SSE subscribers.
- Keep the session log as the durable source; live events are ephemeral.
- Client appends deltas to one assistant bubble and replaces it with the final
  assistant event.
- One prompt at a time; no approvals or writes.

## Acceptance

- [x] A broadcast channel carries runtime events to `/api/events`.
- [x] The SSE test sees `text_delta` before the session write completes.
- [x] The client renders deltas and finalizes on `assistant_message`.
- [x] Tool call events show immediately.
- [x] No duplicate assistant bubble after the turn.
- [x] The endpoint stays read-only and loopback-only.

## Notes

This is a one-way stream. Browser approvals need a request/response channel and
a persistent agent runtime; that is the next slice.
