+++
id = "M9-031"
title = "Multi-line web composer"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-009"]
docs = ["rendering", "adr-0036"]
+++

## Problem

The browser composer was one `<input>` line. Long prompts were hard to write and
there was no newline.

## Scope

- Replace the input with a one-row `<textarea>`.
- Enter sends; Shift+Enter inserts a newline.
- Auto-grow up to 160px; reset after submit.
- Keep the form POST/approval path unchanged.
- Add a visible keyboard hint.

## Acceptance

- [x] Textarea is present and styled like the old input.
- [x] Enter submits; Shift+Enter does not.
- [x] Height grows with content and caps at 160px.
- [x] Submit clears and resets height.
- [x] Static client assertions cover textarea and hint.
- [x] No server/API changes.

## Notes

IME composition is handled by the browser; a future slice can add paste/file
attachments and a submit shortcut hint per platform.
