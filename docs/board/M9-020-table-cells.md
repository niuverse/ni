+++
id = "M9-020"
title = "Inline styles inside table cells"
status = "done"
milestone = "M9"
module = "tui"
size = "M"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-019"]
docs = ["rendering", "adr-0035"]
+++

## Problem

Table cells were flattened to plain text: `**bold**` and `` `code` `` showed
without their styling, unlike the same spans outside tables.

## Scope

- Parse each cell with `markdown::inline`, keeping text and byte ranges.
- Carry `StyledRange` metadata on `TableLine`; width and truncation still use
  plain text.
- Render bold/italic/code/link/strike/math in both inline ratatui and linear
  ANSI modes.
- Truncated cells intentionally drop style ranges rather than misalign them.
- Header bold and border dim continue to apply on top.

## Acceptance

- [x] Cell markers are stripped and style ranges survive alignment.
- [x] Ratatui spans carry `BOLD`/`CODE` as appropriate.
- [x] Linear ANSI emits the same attributes.
- [x] Plain mode emits no escape codes.
- [x] Width/truncation tests still pass.
- [x] PTY table test still passes.

## Notes

Styles are presentation-only and never written to the event log. Cells with
inline styles inside a **truncated** column render plain for that cell.
