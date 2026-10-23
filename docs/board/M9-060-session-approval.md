+++
id = "M9-060"
title = "Allow for this session in web approvals"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-011"]
docs = ["rendering", "adr-0036"]
+++

## Problem

Every mutating tool call forced another approval card. A turn with several
writes became a click loop, unlike the TUI's session-allow choice.

## Scope

- Add a third button: `Allow session`.
- Approval POST accepts `{"id","allowed","session"}`; session allow sets an
  atomic flag.
- The turn approver returns true immediately while the flag is set.
- Reset the flag on new session and switch; deny does not set it.
- Local-only; no persistence across server restarts.
- Test the flag through the HTTP approval path.

## Acceptance

- [x] `Allow session` posts `session: true`.
- [x] The server sets the session flag only when allowed and session are true.
- [x] The approver short-circuits while set.
- [x] New/switch clear the flag.
- [x] Static assertion covers the button.
- [x] Existing approval allow/deny tests pass.

## Notes

**M9-062** scopes this to the specific tool. There is no revoke control yet.
