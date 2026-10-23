+++
id = "M9-023"
title = "Streaming Markdown in the web bubble"
status = "done"
milestone = "M9"
module = "web"
size = "M"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-022"]
docs = ["rendering", "adr-0036"]
+++

## Problem

Text deltas were plain text; the browser only got formatted Markdown when the
turn ended. That is a visible step down from a modern agent surface.

## Scope

- Keep accumulated turn text in the web observer.
- On each `TextDelta`, broadcast the delta plus `html` for the accumulated text.
- Reuse the same Markdown/MathML/code pipeline (escaped HTML).
- Browser updates the streaming bubble's `innerHTML` when `html` is present and
  falls back to `textContent` otherwise.
- Final `assistant_message` still replaces the bubble with durable HTML.

## Acceptance

- [x] SSE `text_delta` carries `html`.
- [x] Browser uses `html` while streaming.
- [x] Raw HTML/code escaping still applies.
- [x] Finalization does not duplicate the bubble.
- [x] A scripted turn asserts `html` over SSE.
- [x] Existing approval/code/math tests pass.

## Notes

The accumulated text is re-rendered per delta for local turn sizes. A token
buffer or incremental renderer is a later optimization; no remote asset.
