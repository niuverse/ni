+++
id = "M9-078"
title = "Throttle streamed Markdown rendering"
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

Every text delta re-rendered the entire accumulated Markdown and sent the full
HTML: O(n²) work for long answers.

## Scope

- Render/send accumulated HTML at most every 100ms or 64 new bytes.
- Other deltas carry only `text`.
- The durable `assistant_message` always renders the final text.
- Client ignores text-only deltas after it has HTML, avoiding a plain-text
  downgrade; before the first HTML it still shows plain text.
- Reset stream render state on final/clear/new bubble.
- Tested by the existing scripted turn (first delta still has HTML).

## Acceptance

- [x] First delta includes HTML.
- [x] Throttled deltas omit HTML.
- [x] Final assistant event has HTML.
- [x] Client does not downgrade rendered HTML.
- [x] Existing streaming tests pass.
- [x] No change to the durable event log.

## Notes

This is a fixed-rate throttle, not an incremental Markdown parser. A future
slice can render only the changed tail.
