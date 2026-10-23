+++
id = "M9-074"
title = "Copy a whole message"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-073"]
docs = ["adr-0036"]
+++

## Problem

Copy only worked per code block or pre. Taking a full user/assistant message
still required manual selection.

## Scope

- Add a `Copy` affordance to user and assistant message bubbles.
- Copy the raw message text, not the rendered HTML/control labels.
- Show it on hover/focus on pointer devices; always visible on narrow screens.
- Reuse the Copied/Copy failed feedback.
- No server change.

## Acceptance

- [x] User and assistant messages get a Copy control.
- [x] Raw text is copied even when HTML is rendered.
- [x] The control is keyboard focusable and visible on mobile.
- [x] Static assertion covers `addMessageCopy`.
- [x] Existing code/pre copy tests pass.

## Notes

Tool cards still use their own Copy on output/commands; the message control does
not include adjacent cards.
