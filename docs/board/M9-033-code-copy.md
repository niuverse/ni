+++
id = "M9-033"
title = "Copy button for web code blocks"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-022"]
docs = ["rendering", "adr-0036"]
+++

## Problem

Code blocks were highlighted but not copyable without selecting text, which is
awkward for long commands and diffs.

## Scope

- After a final assistant HTML render, add a `Copy` button to each `pre.code`.
- Copy the `<code>` text via `navigator.clipboard`.
- Show `Copied` for 1.2s; show `Copy failed` if the API rejects.
- Do not add buttons during every streamed delta.
- No server route, dependency, or remote asset.

## Acceptance

- [x] Every highlighted code block gets a button.
- [x] The button is idempotent when rendering again.
- [x] Clipboard success/failure is visible.
- [x] Static client assertions cover `copy-code`.
- [x] Existing code/Markdown tests pass.

## Notes

Some browsers require a secure context for Clipboard; localhost qualifies.
A future slice can copy whole messages or tool output.
