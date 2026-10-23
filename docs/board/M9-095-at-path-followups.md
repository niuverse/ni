+++
id = "M9-095"
title = "@path attachments and follow-up image history"
status = "done"
milestone = "M9"
module = "tui"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-094"]
docs = ["adr-0037"]
+++

## Problem

Queued follow-ups rebuilt history with the text-only projection, so a resumed
computer-use/multimodal task lost its recent images. TUI users also had to type
a separate `/attach` command.

## Scope

- Keep the attachment store across interactive turns.
- Rebuild follow-up history with `project_history_with_attachments` when a store
  exists.
- Add user-typed `@path` expansion in the TUI; only existing files are consumed,
  missing paths/email handles stay literal.
- Ingest through the same store/budget path and clear after the turn.
- Tests for expansion and existing PTY attach flow.

## Acceptance

- [x] Follow-up history uses the recent-image projection.
- [x] `@existing.png look` attaches and cleans the prompt.
- [x] Missing paths and emails are not swallowed.
- [x] Upload failure leaves the prompt unsent.
- [x] Existing `/attach` PTY test passes.
- [x] Tests/clippy pass.

## Notes

`@path` does not support spaces; `/attach` remains the reliable form for those.
Model output is never scanned for `@path`.
