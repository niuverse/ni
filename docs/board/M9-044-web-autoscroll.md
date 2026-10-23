+++
id = "M9-044"
title = "Follow the latest web message without stealing scroll"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-023"]
docs = ["rendering", "adr-0036"]
+++

## Problem

Streamed output could land below the fold, and a naive always-scroll would
yank the page while the user was reading earlier evidence.

## Scope

- Before rendering an event, check whether the viewport is near the bottom.
- If it is, scroll to the latest after the DOM updates; otherwise leave scroll
  alone.
- Show a floating `↓ Latest` button when not at the bottom.
- Re-evaluate on scroll (passive listener).
- No server, store, or event change.

## Acceptance

- [x] Near-bottom readers follow new output.
- [x] Scrolled-up readers are not moved.
- [x] Latest button appears away from bottom and works.
- [x] Static client assertions cover the helper.
- [x] Existing chat/stream tests pass.

## Notes

The threshold is 160px. Smooth scrolling is used; reduced-motion handling can
be added with `prefers-reduced-motion` later.
