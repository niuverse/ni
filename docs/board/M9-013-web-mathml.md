+++
id = "M9-013"
title = "Math in the web transcript"
status = "done"
milestone = "M9"
module = "web"
size = "M"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-012"]
docs = ["rendering", "adr-0036"]
+++

## Problem

The browser Markdown renderer treated `$...$` as text. The TUI had a Unicode
fallback; the web should typeset math.

## Scope

- Extract inline and display math outside code fences.
- Convert common TeX to native MathML with `latex2mathml`.
- Preserve unmatched `$` (prices) and escaped dollars.
- Insert MathML after Markdown HTML generation so raw-HTML escaping stays on.
- Fall back to escaped `<code class="math">` when conversion fails.
- No remote KaTeX asset or font.

## Acceptance

- [x] Inline math becomes `<math>`.
- [x] Display `$$` becomes block MathML.
- [x] Unmatched currency stays literal.
- [x] Code fences are not math-scanned.
- [x] A conversion failure does not inject raw HTML.
- [x] Browser CSS sizes MathML consistently.

## Notes

Native MathML is the dependency-light v1. KaTeX can be added later as an
optional local asset without changing the stored text or the API shape.
