+++
id = "adr-0035"
title = "ADR-0035: Render Markdown at the terminal boundary"
kind = "adr"
status = "accepted"
order = 62
summary = "The event log stays raw; the transcript parses inline Markdown into styled spans, buffers pipe tables into a block renderer, and leaves math as a readable fallback until a dedicated renderer exists."
+++

## Status

Accepted · 2026-10-23. First slice implemented in `markdown.rs` and `table.rs`
for niucode's TUI.

## Context

The TUI currently prints assistant text verbatim. Users see `**bold**`,
`` `code` ``, and Markdown tables as literal punctuation. A terminal has no DOM:
styling is ANSI spans, width is measured in cells, and a table needs the whole
block before column widths are known.

The event log is the source of truth and must stay unchanged. Rendering is a
presentation concern; the same raw text may later feed a web client, which can
use a real Markdown/KaTeX renderer.

## Decision

1. Parse **inline** Markdown into spans (`bold`, `italic`, `code`, `link`,
   `strike`) before styling. Unbalanced markers stay literal while streaming.
2. Strip ATX heading markers and render the heading with the accent style.
   List markers become bullets/tasks, and `>` becomes a muted quote bar.
3. Buffer consecutive pipe-table lines and render them as a box table only when
   the block ends. Column widths are cell-width based and capped to the
   viewport.
4. Code fences bypass Markdown parsing and keep the existing syntax highlight;
   the fence markers are replaced by a `▎ language` label.
5. Math uses a readable Unicode fallback for common TeX (`$...$`, `$$...$$`,
   `\alpha`, `\frac`, `\sqrt`); `$$` blocks are buffered and lose their
   fences, and unmatched `$` stays literal. A web surface may render full TeX.
   No terminal TeX engine is promised.
6. Rendering never writes to the session log and never trusts model-provided
   ANSI; existing sanitization stays.
7. Styling must preserve text width: styles change attributes, not characters,
   except for the deliberate removal of Markdown markers.

## Consequences

**Better**

- Readable emphasis, code, headings, links, and tables in the TUI and linear
  fallback.
- One parser feeds ratatui spans and linear ANSI, so the two paths stay close.
- The raw log remains portable to a future web renderer.

**Worse**

- Tables delay until the block ends; a streaming table appears at once.
- Math is literal until the web renderer exists.
- Very wide tables truncate with `…` rather than horizontal scrolling.

## Follow-ups

- Lists, block quotes, and task checkboxes.
- Table cell styling and alignment for inline spans.
- Streaming animation and progress polish (separate ADR/slice).
- Web surface with Markdown + KaTeX; see [rendering](../rendering.md).
