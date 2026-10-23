+++
id = "M9-120"
title = "Structured computer action audit event"
status = "done"
milestone = "M9"
module = "loop"
size = "M"
priority = "P1"
updated = "2026-10-24"
completed = "2026-10-24"
depends_on = ["M9-099"]
docs = ["adr-0037"]
+++

## Problem

Computer actions were only inferable from AssistantMessage tool calls,
Approval, and ToolResult. There was no single structured record binding action,
target, observation, purpose, and outcome.

## Scope

- Add `Event::ComputerAction` with action label, window id, observation id,
  sanitized purpose, ok/error, and type length + SHA-256.
- Emit one event for every successfully parsed computer action, including
  policy/approval denials and backend errors.
- Never store typed text or screenshots in this event.
- Append after the action execution; parse failures have no action to audit.
- Ignore the event in context projection.
- Test click + type records and typed redaction metadata.

## Acceptance

- [x] One structured event per parsed action.
- [x] Type audit has chars + SHA-256, no text.
- [x] Click audit records window/observation.
- [x] Purpose and error are bounded/sanitized.
- [x] Projection ignores it; old logs remain readable.
- [x] Tests/clippy pass.

## Notes

Observe has no model-supplied observation id; its captured id remains in the
ToolResult. A later slice can include it in the audit event.
