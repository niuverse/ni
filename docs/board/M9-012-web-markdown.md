+++
id = "M9-012"
title = "Markdown in the web transcript"
status = "done"
milestone = "M9"
module = "web"
size = "M"
priority = "P0"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-011"]
docs = ["rendering", "adr-0036"]
+++

## Problem

The browser rendered every assistant message as `textContent`. The TUI had
rich Markdown, but the web surface did not.

## Scope

- Server-render assistant `text` to HTML with pulldown-cmark.
- Enable tables and strikethrough; escape raw HTML and block `javascript:` links.
- Include `html` in `/api/session` and SSE assistant events.
- Browser replaces the streaming bubble with rendered HTML at turn end.
- Keep plain text during deltas; no remote assets or client build.

## Acceptance

- [x] Markdown HTML includes `<strong>` and `<table>`.
- [x] Raw `<script>` is escaped, not executed.
- [x] `javascript:` hrefs are blocked.
- [x] SSE/session events carry `html` for assistant messages.
- [x] Browser uses `innerHTML` only for server-rendered HTML.
- [x] CSS styles headings, lists, blockquotes, tables, and code.

## Notes

Math is still the Unicode/raw text in the web client; KaTeX is the next slice.
Streaming deltas stay plain text and are replaced by the final HTML.
