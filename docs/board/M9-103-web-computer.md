+++
id = "M9-103"
title = "Web computer policy and status"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-099"]
docs = ["adr-0037"]
+++

## Problem

The browser surface could send prompts and approvals, but it had no computer
policy entry and no visible computer state.

## Scope

- Add `niucode web --computer off|observe|control|autonomous`.
- Build and arm the platform runtime once at server start.
- Attach it to every browser prompt's `AgentConfig`.
- Include `{policy, armed}` in `/api/status`.
- Show a computer status chip in the header and `/status` output.
- Keep `off` the default and the runtime absent unless configured.
- Web approvals continue to gate `control` input.

## Acceptance

- [x] `/api/status` reports computer off by default.
- [x] Configured policy is reflected in the chip.
- [x] Browser prompts receive the runtime.
- [x] Observation/input still go through policy and web approvals.
- [x] Static/status tests pass.
- [x] No control endpoint is implied.

## Notes

Arm/stop from the browser is a later slice. The runtime is process-scoped and
mirrors the TUI/REPL policy.
