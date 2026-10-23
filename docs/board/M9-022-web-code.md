+++
id = "M9-022"
title = "Highlighted code in the web transcript"
status = "done"
milestone = "M9"
module = "web"
size = "M"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-021"]
docs = ["rendering", "adr-0035", "adr-0036"]
+++

## Problem

The browser rendered fenced code as an unstyled `<pre><code>` block, even
though the TUI had token highlighting.

## Scope

- Extract fenced code before Markdown parsing.
- Render `<pre class="code"><code class="language-…">` with token spans from
  the same line classifier.
- Escape every token; sanitize the language class.
- Insert generated blocks after Markdown HTML generation, keeping raw-HTML
  escaping on.
- Style keywords, strings, comments, numbers, and functions in the embedded
  CSS.

## Acceptance

- [x] Highlighted HTML includes token classes.
- [x] `<script>` inside a fence is escaped.
- [x] Fence markers do not reach the browser.
- [x] Language class is sanitized.
- [x] No remote syntax-highlighting asset.
- [x] Existing Markdown/MathML/approval tests pass.

## Notes

The highlighter is line-based and shares niucode's token classifier. Multi-line
comments/strings remain a later upgrade.
