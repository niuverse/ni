+++
id = "M9-098"
title = "Computer tool contract and session controller"
status = "done"
milestone = "M9"
module = "tools"
size = "M"
priority = "P0"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-097"]
docs = ["adr-0037"]
+++

## Problem

The X11 backend could execute actions, but there was no model-facing tool
schema, typed input parser, observation registry, or session policy gate.

## Scope

- Add `tool_spec()` JSON schema for observe/click/type/key/scroll.
- Add `parse_action()` with stable field errors.
- Add `ComputerSession` with policy/arm state and an eight-observation
  in-memory registry.
- Resolve policy from the observation's window kind; unarmed sessions deny.
- Add `ComputerBackend` trait and `Capture` type; X11 implements the trait.
- No agent loop wiring or provider image delivery yet.

## Acceptance

- [x] Tool schema enumerates exactly the five actions.
- [x] Click/key/scroll inputs parse; missing/unknown fields fail.
- [x] Unarmed/off sessions deny observation and input.
- [x] Unknown observations fail rather than using stale state.
- [x] Expired observations fail validation.
- [x] Autonomous uses the observed window kind.
- [x] X11 satisfies the backend trait.
- [x] Tests/clippy pass.

## Notes

The next slice owns tool-result image delivery and the approval bridge in the
agent loop. Screenshots remain ephemeral until that path exists.
