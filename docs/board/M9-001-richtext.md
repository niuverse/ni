+++
id = "M9-001"
title = "Inline Markdown and tables"
status = "done"
milestone = "M9"
module = "tui"
size = "M"
priority = "P0"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M6-001"]
docs = ["rendering", "adr-0035"]
+++

## Problem

Assistant text reached the terminal verbatim: `**bold**`, backticks, and pipe
tables appeared as punctuation, and headings kept their `#`.

## Scope

- Parse inline bold/italic/code/link/strike into styled spans.
- Strip ATX heading markers and use the accent style.
- Buffer pipe tables and render box borders, alignment, and a width cap.
- Keep the same spans for ratatui and linear ANSI.
- Add a PTY test proving markers are gone and the table is boxed.

## Acceptance

- [x] `**bold**`, `*italic*`, and `` `code` `` remove their markers and style.
- [x] Unbalanced markers stay literal while streaming.
- [x] ATX headings render without `#`.
- [x] Pipe tables render as `┌ ├ │` blocks with right alignment.
- [x] Wide tables truncate to the viewport with `…`.
- [x] PTY output contains no `**bold**` or raw header row.

## Notes

This is P1 of [rendering](../rendering.md). Math is still literal; cell
styling, lists, and motion are later slices.
