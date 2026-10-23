+++
id = "M9-003"
title = "Lists, tasks, and quotes"
status = "done"
milestone = "M9"
module = "tui"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-002"]
docs = ["rendering", "adr-0035"]
+++

## Problem

Markdown list markers, task checkboxes, and block quotes reached the terminal
verbatim. `+` was especially risky because diff output also uses it.

## Scope

- `- ` and `* ` become `•`, preserving indentation.
- `- [ ]` / `- [x]` become `☐` / `☑`.
- `> ` becomes `│`, with muted text.
- Do not treat `+ ` as a list marker; diff lines stay diff lines.
- Parse inline emphasis inside list text.

## Acceptance

- [x] Bullets, nested indentation, tasks, and quotes transform.
- [x] `*italic*` and `+ new line` are not lists.
- [x] Quotes use the muted tone.
- [x] PTY output contains `• first item`, `☐ pending task`, and `│ quoted line`.
- [x] Existing diff tone tests still pass.

## Notes

Ordered lists keep their `1.` marker for copyability. Horizontal rules and
nested task indentation are later polish.
