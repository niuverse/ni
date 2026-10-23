+++
id = "M9-005"
title = "Display math blocks"
status = "done"
milestone = "M9"
module = "tui"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-002"]
docs = ["rendering", "adr-0035"]
+++

## Problem

Inline `$…$` had a Unicode fallback, but display math:

```text
$$
\alpha + \beta = \gamma
$$
```

still printed dollar fences and TeX commands.

## Scope

- Buffer a `$$` block until its closing `$$`.
- Drop the fences and render each line with the Unicode fallback.
- Prefix the block with `ƒ` and keep it readable in linear and inline TUI.
- Flush an unterminated block at turn end instead of losing it.
- Inline `$$…$$` on one line still uses the inline path.

## Acceptance

- [x] `$$` fences do not reach the terminal.
- [x] `\alpha + \beta = \gamma` renders as `α + β = γ`.
- [x] A missing closing fence flushes at completion.
- [x] PTY test asserts the symbols and no `$$`.
- [x] Existing inline math and currency tests still pass.

## Notes

Matrices, aligned environments, and multi-line TeX semantics are intentionally
not interpreted; KaTeX on the web surface owns real typesetting.
