+++
id = "M9-061"
title = "Approval timeout countdown"
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

The server fails closed after 300s, but the approval card showed no deadline. A
user could leave a decision too long and only discover it via a failed turn.

## Scope

- Show `expires in 5:00`, ticking once per second.
- Disable Allow/Allow session/Deny at zero and show `approval expired`.
- Clear the timer on any decision.
- Keep the server's 300s denial authoritative.
- No new endpoint or payload field.

## Acceptance

- [x] Countdown matches the 300s server timeout.
- [x] Buttons disable at zero.
- [x] A decision clears the interval.
- [x] Static client assertion covers the countdown.
- [x] Existing approval/session tests pass.

## Notes

The client timer is advisory; the approver still blocks and fails closed on the
server side even if the page is closed.
