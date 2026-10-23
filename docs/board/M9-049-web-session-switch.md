+++
id = "M9-049"
title = "Switch archived sessions from web"
status = "done"
milestone = "M9"
module = "web"
size = "M"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-048"]
docs = ["rendering", "adr-0036"]
+++

## Problem

Archived threads were visible but inert. Selecting a chapter required leaving
the browser and manipulating files.

## Scope

- Add `Options.switch_session_provider` (id → allowlisted path).
- `POST /api/switch {"id"}` updates the active path and snapshot; 400 on bad id,
  403 when disabled, 409 while a turn runs.
- Context sections with `kind: "threads"` render as buttons.
- Clicking asks for confirmation, posts, and reloads onto the new SSE path.
- Embedder owns validation and archive/move semantics.

## Acceptance

- [x] Provider-backed switch returns 200 and changes the active path.
- [x] Switch snapshot covers the target events.
- [x] Disabled switch fails closed.
- [x] Bad ids are rejected by the provider.
- [x] Thread section renders buttons and confirmation.
- [x] Existing new/clear/context tests pass.

## Notes

The server does not infer thread ids or touch the filesystem; it only invokes the
embedder provider. niuse archives the active thread and moves the selected
chapter back to `thread.jsonl`.
