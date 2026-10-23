+++
id = "M9-073"
title = "Copy tool output and commands"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-033"]
docs = ["adr-0036"]
+++

## Problem

Copy existed only on final assistant code blocks. Tool output and approval
commands still required manual selection.

## Scope

- Generalize Copy to `pre.code, pre.copyable`.
- Mark tool-result and command-preview blocks copyable.
- Snapshot the raw text before inserting the button, so the button label is not
  copied.
- Keep one button per block and the Copied/Copy failed feedback.
- No server change.

## Acceptance

- [x] Tool results get a Copy button.
- [x] Command previews get a Copy button.
- [x] Copied text excludes the button label.
- [x] Buttons remain idempotent.
- [x] Static assertion covers `pre.copyable`.
- [x] Existing code-copy tests pass.

## Notes

The copy source is the rendered text, not the raw event field; ANSI-free output
and diffs are copied as displayed.
