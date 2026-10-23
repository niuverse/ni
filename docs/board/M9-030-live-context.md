+++
id = "M9-030"
title = "Live product context and remembered sections"
status = "done"
milestone = "M9"
module = "web"
size = "M"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-029"]
docs = ["rendering", "adr-0036"]
+++

## Problem

`/api/context` was a one-shot snapshot. After a turn changed memory, goals, or
metrics, the sidebar stayed stale. Collapsing a section was forgotten on reload.

## Scope

- Add optional `Options.context_provider`, called per `/api/context` request.
- Static `context` remains the fallback.
- Client refreshes context after `turn_end` and every 15s.
- Collapsed sections persist in `localStorage`, keyed by title.
- Filter state and escaping remain unchanged.
- No new write path or remote asset.

## Acceptance

- [x] A provider is called on each context request (loopback test).
- [x] Static context still works without a provider.
- [x] Browser refreshes after a turn and on interval.
- [x] Collapse state persists across reloads.
- [x] Section content stays escaped.
- [x] Existing sidebar/filter tests pass.

## Notes

Refresh is polling, not SSE; the context payload is small enough for now. A
future `context_changed` event could replace the timer.
