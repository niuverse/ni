+++
id = "M9-011"
title = "Browser approvals"
status = "done"
milestone = "M9"
module = "web"
size = "M"
priority = "P0"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-010"]
docs = ["rendering", "adr-0036"]
+++

## Problem

The web turn was hard-coded `approval never` and read-only. A browser client
could not allow a write; the TUI and web surfaces behaved differently.

## Scope

- Web turns use `workspace-write`, network off, and `ApprovalMode::Ask`.
- The approver sends an SSE `approval` event with an ID, tool, summary, and
  serialized preview, then blocks.
- `POST /api/approval {"id","allowed"}` releases that exact turn.
- Unknown or stale IDs return 404; the pending map is cleared after use.
- Browser card shows the preview and Allow/Deny.
- One prompt at a time; timeout denies after 300s.

## Acceptance

- [x] Approval preview types serialize.
- [x] SSE carries the approval ID and preview.
- [x] POST approval releases the blocked approver (unit).
- [x] A scripted write turn is denied without approval and writes after Allow.
- [x] The browser renders Allow/Deny and reports the result.
- [x] Loopback-only and one-turn-at-a-time remain intact.

## Notes

Approval is exposed only through the local HTTP endpoint; there is no remote
auth or token. A future remote mode needs its own threat model. The 300s timeout
fails closed.
