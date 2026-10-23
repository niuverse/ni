+++
id = "M9-026"
title = "Multi-line code comment state"
status = "done"
milestone = "M9"
module = "tui"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-021"]
docs = ["rendering", "adr-0035"]
+++

## Problem

The syntax classifier reset on every line, so an opening `/*` did not color the
following lines. TUI and web code looked wrong for block comments.

## Scope

- Add a stateful `Highlighter` with a block-comment flag.
- `/* … */` spans lines; closing comment characters are consumed and parsing
  resumes on the same line.
- Reset state when a new code fence opens.
- Web renders a whole block with one highlighter; TUI carries state per code
  block.
- Preserve exact text and width.

## Acceptance

- [x] Two-line block comments are classified as comments.
- [x] Code after `*/` is parsed as code again.
- [x] Rebuilt text equals the input.
- [x] TUI PTY fixture includes a multi-line comment.
- [x] Web test counts multiple `tok-comment` spans.
- [x] Existing single-line code tests pass.

## Notes

Strings and triple-quoted comments are still line-based; this slice only adds
Rust/C-style block comments.
