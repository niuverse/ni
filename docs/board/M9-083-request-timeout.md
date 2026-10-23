+++
id = "M9-083"
title = "Request body read timeout"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-082"]
docs = ["adr-0036"]
+++

## Problem

A client could send headers with a large `Content-Length` and then stall,
holding a server task and socket open indefinitely.

## Scope

- Wrap request reads in a 5s timeout (200ms in tests).
- Return `408 Request Timeout` JSON.
- Keep the 1MiB body cap and complete-body read.
- Test a stalled partial body.
- No change to SSE/turn execution timeouts.

## Acceptance

- [x] Stalled body returns 408.
- [x] Test server responds within its 200ms test timeout.
- [x] Normal complete bodies still work.
- [x] Existing HTTP/token/approval tests pass.
- [x] No new dependency.

## Notes

This bounds request ingestion, not model/tool execution. SSE streams and turns
keep their own lifecycle.
