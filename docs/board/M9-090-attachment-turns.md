+++
id = "M9-090"
title = "Attachments in a turn"
status = "done"
milestone = "M9"
module = "context"
size = "M"
priority = "P0"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-089"]
docs = ["adr-0037"]
+++

## Problem

Metadata on a user event was not enough: the provider request still carried
text only, budgets were not checked at send time, and text/image attachments had
no message projection.

## Scope

- Add `run_with_attachments` without changing existing run APIs.
- Check the per-turn budget before appending the user event.
- Append attachment refs to the durable user event.
- Images become OpenAI-compatible data-URL parts.
- Text attachments become fenced `<attachment>` context.
- Add an end-to-end scripted test: request parts, event metadata, final text.
- No UI attachment entry yet.

## Acceptance

- [x] Budget is checked before the turn is recorded.
- [x] Event carries the same two refs.
- [x] Provider receives a text + image content array.
- [x] Text attachment content appears in fenced context.
- [x] Existing `run`/`run_with_history` behavior is unchanged.
- [x] Tests/clippy pass.

## Notes

Historical/recent-image projection is the remaining context slice; this card
only guarantees the current turn.
