+++
id = "M9-094"
title = "TUI attach command and pending chips"
status = "done"
milestone = "M9"
module = "tui"
size = "M"
priority = "P0"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-093"]
docs = ["adr-0037", "adr-0035"]
+++

## Problem

The TUI had no attachment entry; the only way to send an image was the CLI or
web client.

## Scope

- Add `/attach <path>` and `/detach [index]` slash commands.
- Ingest into the session attachment store; auto-classify image/text.
- Show pending chips with kind, name, size.
- Enforce the existing per-turn budget before sending.
- Pass pending refs through `live::run`/`run_interactive_with_attachments` or
  the legacy callback path to `run_with_attachments`.
- Clear pending attachments after the turn.
- PTY test: attach a PNG, send a prompt, assert the session log carries the ref.

## Acceptance

- [x] `/attach` parses paths and shows chips.
- [x] `/detach` removes one or all pending attachments.
- [x] Budget errors do not silently send.
- [x] Both editor/live and legacy TUI paths consume attachments.
- [x] Event/user message carries the ref.
- [x] Existing Markdown PTY assertions still pass.
- [x] TUI tests/clippy pass.

## Notes

Queued follow-up inputs remain text-only in this slice; the next task handles
their attachment-aware history. Terminal drag/drop is intentionally not
claimed.
