+++
id = "M9-028"
title = "Triple-quoted multi-line strings"
status = "done"
milestone = "M9"
module = "tui"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-026"]
docs = ["rendering", "adr-0035"]
+++

## Problem

Block comments spanned lines, but Python-style `"""…"""` / `'''…'''`
docstrings reset on every line and were highlighted as code.

## Scope

- Track an optional triple-quote in the stateful highlighter.
- Opening and closing triples stay one `String` token across lines.
- Code after the closing triple on the same line resumes normal parsing.
- TUI and web share the state; exact text and width are preserved.
- Single-quoted strings and backticks keep the existing line behavior.

## Acceptance

- [x] Triple-quoted bodies are `String` tokens across lines.
- [x] Closing triple resumes parsing; following numbers classify.
- [x] Rebuilt text equals the input.
- [x] Web test finds multiple `tok-string` spans.
- [x] Existing block-comment/code tests pass.

## Notes

Raw strings (`r#"…"#`), nested triples, and multi-line backticks are still out
of scope.
