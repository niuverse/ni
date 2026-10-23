+++
id = "M9-019"
title = "Table header and border tone"
status = "done"
milestone = "M9"
module = "tui"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-001"]
docs = ["rendering", "adr-0035"]
+++

## Problem

Tables had the right geometry but the header and borders were visually flat.
A dense table was hard to scan.

## Scope

- Return `TableLine` metadata (`Border`, `Header`, `Row`) from the table
  renderer.
- Bold the header row and dim borders in the theme.
- Preserve exact cell widths; style only attributes, never characters.
- Apply the same tone in inline ratatui and linear ANSI modes.
- Keep plain mode free of escape sequences.

## Acceptance

- [x] Header spans carry `BOLD`; borders carry `DIM`.
- [x] Plain mode carries no modifiers.
- [x] Table geometry tests still pass.
- [x] PTY table test still passes.
- [x] No ANSI reaches the transcript or event log.

## Notes

Inline Markdown inside cells is still stripped. Per-cell styling requires a
structured cell model and remains a later pass.
