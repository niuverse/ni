+++
id = "M9-016"
title = "Web status strip"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-015"]
docs = ["rendering", "adr-0036"]
+++

## Problem

The browser showed the transcript but not the model, working directory, or
policy it was running under. A harness needs that at a glance.

## Scope

- `GET /api/status` returns brand, provider, model, cwd, sandbox, approval,
  network, and busy.
- Header chips show model, cwd, and policy.
- Refresh every 2.5s; narrow screens hide cwd/policy.
- No new write path or remote asset.

## Acceptance

- [x] Status endpoint is loopback-only and tested.
- [x] It reports workspace-write / ask / off.
- [x] Chips populate from the endpoint.
- [x] The client handles a missing endpoint.
- [x] Layout stays readable on narrow screens.

## Notes

Busy is polled rather than pushed. A later slice can fold it into SSE phase
events and show a turn timer.
