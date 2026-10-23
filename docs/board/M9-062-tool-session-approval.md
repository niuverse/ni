+++
id = "M9-062"
title = "Scope session approval to the tool"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-060"]
docs = ["rendering", "adr-0036"]
+++

## Problem

`Allow session` approved every subsequent tool, which is broader than the TUI's
per-tool session allow and riskier than the user expects.

## Scope

- Track the tool with each pending approval.
- Session allow adds only that tool to an allowlist.
- The approver short-circuits only when `request.tool` is in the set.
- New/switch clears the allowlist.
- Button copy becomes `Allow tool`.
- No blanket boolean remains.

## Acceptance

- [x] Pending approvals carry the tool name.
- [x] Session approval stores the tool, not a blank cheque.
- [x] The approver checks the requested tool.
- [x] New/switch clear the allowlist.
- [x] HTTP approval test asserts the tool entry.
- [x] Static assertion covers `Allow tool`.

## Notes

The allowlist is in-memory and loopback-only. Per-path or per-command scopes
would require expanding the approval request payload.
