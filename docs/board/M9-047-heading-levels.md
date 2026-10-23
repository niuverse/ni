+++
id = "M9-047"
title = "Distinct TUI heading levels"
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

Every ATX heading rendered identically: accent, bold. A long Markdown document
had no visible hierarchy.

## Scope

- Preserve the heading level in the parser (`heading_with_level`).
- Keep `heading()` as the text-only wrapper for existing callers.
- Theme:
  - `#` → accent bold + underline
  - `##` → accent bold
  - `###`+ → cyan, not bold
- Text and marker stripping are unchanged.
- Plain mode emits no attributes.

## Acceptance

- [x] Levels 1/3 parse correctly; level 7 and `#hashtag` do not.
- [x] H1 carries underline; H3 loses bold.
- [x] Content stays exactly the heading text.
- [x] Existing heading/table/PTY tests pass.
- [x] No event-log change.

## Notes

Underline is ANSI/ratatui presentation only. Terminal themes may render it
differently, but text and width are stable.
