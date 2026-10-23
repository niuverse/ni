+++
id = "M9-036"
title = "Reject unknown web slash commands locally"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-035"]
docs = ["rendering", "adr-0036"]
+++

## Problem

`/foo` was sent to the model, unlike the TUI, which treats unknown slash
commands as local input errors. That spent a turn and could confuse the agent.

## Scope

- Recognize known local commands first.
- Any other `/…` prints `unknown command · /help` locally.
- Do not POST, do not start a turn, do not write the session.
- Keep the input cleared and height reset.
- Keep known command behavior unchanged.

## Acceptance

- [x] Unknown slash commands never call `/api/prompt`.
- [x] The message points at `/help`.
- [x] Known commands still work.
- [x] Static client assertions cover the path.
- [x] Existing prompt/approval tests pass.

## Notes

This matches the TUI's "unknown /foo is not model input" rule. `/clear` is still
intentionally unavailable until real session semantics exist.
