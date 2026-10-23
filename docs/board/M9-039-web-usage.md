+++
id = "M9-039"
title = "Token usage chip in the web header"
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

The browser showed elapsed time but not how many tokens a turn used or how
much the page had consumed. A harness should make cost visible.

## Scope

- Accumulate `input_tokens`, `output_tokens`, and `cached_tokens` from
  `turn_end` events.
- Show a compact `N in / N out` chip; tooltip shows cached input.
- Load initial session usage as its events stream in.
- Keep totals page-local; no server state or pricing claims.
- Reset on a page reload.

## Acceptance

- [x] The header has a usage chip.
- [x] Amounts compact to `k`/`m`.
- [x] Cached tokens are available in the tooltip.
- [x] Initial session events contribute.
- [x] Static client assertions cover usage.
- [x] No API or store changes.

## Notes

Page-local totals are not the same as a session-wide billing ledger. A future
slice can ask `/api/status` for durable usage.
