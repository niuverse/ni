+++
id = "M9-018"
title = "Ordered lists and horizontal rules"
status = "done"
milestone = "M9"
module = "tui"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-003"]
docs = ["rendering", "adr-0035"]
+++

## Problem

Bullets, tasks, and quotes were rendered, but `1. item` and `---` stayed raw.
Markdown documents therefore looked inconsistent in the transcript.

## Scope

- Recognize `N.` and `N)` ordered markers, preserving the number.
- Recognize `---`, `***`, and `___` (three or more) as a horizontal rule.
- Render rules as a muted `─` line; leave ordered text readable.
- Keep tables and code fences out of the rule/order path.
- Do not treat `+` diff lines as lists.

## Acceptance

- [x] `1. numbered` and `2) next` keep their numbers.
- [x] `---` and `***` become a rule, not raw dashes.
- [x] Rule text is muted in the theme.
- [x] PTY fixture waits for numbered lines and `────`.
- [x] Existing bullets/tasks/quotes/table tests pass.

## Notes

The rule is a fixed 40 cells, not viewport-width; terminal wrapping still keeps
it safe. Table header/cell styling remains a later pass.
