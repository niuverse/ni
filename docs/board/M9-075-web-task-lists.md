+++
id = "M9-075"
title = "Task lists in web Markdown"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-012"]
docs = ["rendering", "adr-0036"]
+++

## Problem

The TUI renders `- [ ]` / `- [x]` as task checkboxes; the web renderer left them
as literal Markdown because task lists were not enabled.

## Scope

- Enable `ENABLE_TASKLISTS` in pulldown-cmark.
- Render disabled checkboxes with aligned margins.
- Test checked/unchecked output.
- No server/API change.

## Acceptance

- [x] `- [x]` produces a checked disabled checkbox.
- [x] `- [ ]` produces an unchecked disabled checkbox.
- [x] List spacing remains readable.
- [x] Existing Markdown/table/code tests pass.
- [x] No interactive mutation of task state.

## Notes

This is display-only. Editing task state from the browser would require a
Markdown write-back path and is out of scope.
