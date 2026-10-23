+++
id = "M9-099"
title = "Computer tool in the agent loop"
status = "done"
milestone = "M9"
module = "tools"
size = "M"
priority = "P0"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-098"]
docs = ["adr-0037"]
+++

## Problem

The tool contract existed, but the agent did not advertise or dispatch
`computer`, observations never reached the model as images, and input actions
had no approval bridge.

## Scope

- Add `AgentConfig.computer` with a `ComputerRuntime` (backend + session).
- Advertise the `computer` tool when configured.
- Observe: capture, remember, return metadata plus a PNG data URL as tool
  content parts.
- Input: resolve policy from the observation, request approval when required,
  append `Event::Approval`, validate immediately before XTEST.
- Convert backend/observation failures into failed ToolResults, not aborted
  turns.
- Add `niucode exec --computer off|observe|control|autonomous` (Linux X11,
  arms the session; connects to `$DISPLAY`).
- Fake-backend tests for observe image parts and input approval; CLI parsing test.

## Acceptance

- [x] `computer` is advertised and dispatched.
- [x] Observe tool message carries an image part.
- [x] Control input asks for approval and records allowed/denied.
- [x] Backend/validation errors become tool failures.
- [x] CLI accepts the four policies.
- [x] X11 connection failure warns instead of crashing.
- [x] Tests/clippy pass.

## Notes

TUI `--computer` entry and arm/stop UI are next; autonomous red lines are
enforced by the session controller. The model must be vision-capable for
observe screenshots.
