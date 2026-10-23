+++
id = "M9-021"
title = "Code token highlighting"
status = "done"
milestone = "M9"
module = "tui"
size = "M"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-004"]
docs = ["rendering", "adr-0035"]
+++

## Problem

Fenced code was tokenized only by punctuation and a flat keyword list. Strings,
comments, numbers, and function calls were not distinguished, so code blocks
were hard to scan.

## Scope

- Add a dependency-free line classifier (`syntax`) with token classes:
  keyword, string, comment, number, function, plain.
- Handle `//` and leading `#` comments, quoted strings with escapes, numeric
  literals, identifiers, and function calls.
- Theme mapping: keywords magenta, strings green, comments dim, numbers yellow,
  functions cyan; plain stays default.
- Preserve exact text; classification never changes width.
- Use the same spans for inline and linear renderers.

## Acceptance

- [x] Keyword/string/comment/number classification has unit tests.
- [x] Rebuilt tokens equal the input for representative lines.
- [x] Code rows carry the expected colors; plain mode carries none.
- [x] Existing code/markdown tests pass.
- [x] No new dependency.

## Notes

The classifier is line-based and language-agnostic. Multi-line strings, nested
block comments, and language-specific grammars are out of scope; this is a
readability aid, not a compiler.
