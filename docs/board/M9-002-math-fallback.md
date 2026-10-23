+++
id = "M9-002"
title = "Terminal math fallback"
status = "done"
milestone = "M9"
module = "tui"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-001"]
docs = ["rendering", "adr-0035"]
+++

## Problem

`$E = mc^2$` and `$\alpha \le \beta$` reached the terminal with dollar signs
and TeX commands. A terminal cannot typeset TeX, but it can do better than raw
source.

## Scope

- Parse inline `$...$` and `$$...$$` when closed on the same line.
- Convert common symbols (`\alpha`, `\le`, `\sum`, `\to`, …) to Unicode.
- Convert simple `\frac{a}{b}` to `a/b` and `\sqrt{x}` to `√(x)`.
- Keep unmatched `$` literal (for prices).
- Style math distinctly; the web renderer will use KaTeX.

## Acceptance

- [x] `$E = mc^2$` renders without dollar signs.
- [x] `\alpha \le \beta` renders as `α ≤ β`.
- [x] `\frac{1}{2}` and `\sqrt{x}` render readably.
- [x] `costs $5 today` stays literal.
- [x] The PTY test waits for `α` and `≤`.

## Notes

This is a readable fallback, not a TeX engine. Complex fractions, matrices, and
multi-line display math remain raw; P4's web surface uses KaTeX for those.
