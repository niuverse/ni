+++
id = "M9-106"
title = "Web computer arm and stop"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-24"
completed = "2026-10-24"
depends_on = ["M9-103"]
docs = ["adr-0037"]
+++

## Problem

The browser could display computer policy but could not arm or stop it; an
operator had to return to the TUI/REPL or restart the server.

## Scope

- Add `POST /api/computer` with `{action:"arm"|"stop"}`.
- Reuse loopback Host/Origin guards and the per-run token/cookie gate.
- `stop` calls `ComputerRuntime::emergency_stop` (policy off, disarm, release).
- `arm` only re-arms the configured policy; off returns 409.
- Return the current `{policy, armed, configured}` status.
- Make the header chip a button that toggles arm/stop when configured.
- Keep policy switching out of the browser.
- Test the unconfigured 409 and static button wiring.

## Acceptance

- [x] Arm/stop requires the same auth as prompts/approvals.
- [x] Stop releases input and turns policy off.
- [x] Off/unconfigured servers cannot arm.
- [x] The chip is disabled when no runtime is configured.
- [x] Status is refreshed after the action.
- [x] Tests/clippy pass.

## Notes

This is a process-scoped emergency control, not a policy editor. Starting with
`--computer off` still requires a restart to add a backend.
