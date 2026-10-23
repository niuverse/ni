+++
id = "M9-081"
title = "TUI alert quote tones"
status = "done"
milestone = "M9"
module = "tui"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-080"]
docs = ["rendering", "adr-0035"]
+++

## Problem

Alert quotes were labeled but all rendered with the muted quote tone, so a
warning looked like a note.

## Scope

- Map note → accent, tip → success, important → magenta, warning → warning,
  caution → danger.
- Keep ordinary quotes muted.
- Keep the `│` bar and alert label.
- No parser change.
- Existing block/terminal tests must pass.

## Acceptance

- [x] Warning and caution differ from note/tip.
- [x] Ordinary quotes stay muted.
- [x] Labels remain `note ·`/`warning ·` etc.
- [x] Workspace tests/clippy pass.
- [x] No raw ANSI or width change.

## Notes

The whole quote line, including the bar, takes the variant tone. A future slice
could color only the label.
