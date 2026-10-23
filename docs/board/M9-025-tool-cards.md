+++
id = "M9-025"
title = "Web tool call cards"
status = "done"
milestone = "M9"
module = "web"
size = "M"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-010"]
docs = ["rendering", "adr-0036"]
+++

## Problem

Tool calls were one inline line with stringified arguments; results were a
separate disclosure. Dense turns were hard to scan and the two were not
visually associated.

## Scope

- Render a collapsible card per tool call: name + bounded input summary, full
  pretty input inside.
- Render a result card with `✓`/`✗`, tool name, summary, and output.
- Escape every value with `textContent`.
- Cap output height and scroll; do not write cards to the session.
- Keep final assistant `tool_calls` metadata as a fallback.

## Acceptance

- [x] Tool calls and results render as cards.
- [x] OK/error states are visually distinct.
- [x] Input/output are escaped, bounded, and scrollable.
- [x] The cards are collapsed by default.
- [x] Static client assertions cover the renderers.
- [x] No new server endpoint or write path.

## Notes

Pairing is by event order/name; the runtime does not carry a stable call id to
the browser yet. That is a later protocol slice.
