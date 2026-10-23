+++
id = "M9-121"
title = "Observe audit records the captured target"
status = "done"
milestone = "M9"
module = "loop"
size = "S"
priority = "P1"
updated = "2026-10-24"
completed = "2026-10-24"
depends_on = ["M9-120"]
docs = ["adr-0037"]
+++

## Problem

`computer_action` for observe had an empty observation id and often window 0,
because the model does not supply those fields for observe. The captured target
only existed in the ToolResult text.

## Scope

- Audit observe success with the backend's captured window id and observation
  id.
- Audit observe failures/denials through the wrapper with whatever target the
  model supplied.
- Keep exactly one action event per parsed observe.
- No screenshot data or base64 in the event.
- Regression asserts window 7 + observation `obs-1`.

## Acceptance

- [x] Observe success records captured window/observation.
- [x] No duplicate event on success.
- [x] Failure/denial still records a failed action.
- [x] No image data stored.
- [x] Tests/clippy pass.

## Notes

The event remains metadata-only; the ToolResult carries dimensions/expiry, and
`--computer-keep-frames` is the only durable screenshot path.
