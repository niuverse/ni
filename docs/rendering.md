+++
id = "rendering"
title = "Rendering and experience roadmap"
kind = "plan"
status = "active"
order = 63
summary = "Phased plan for rich terminal output, motion, and a local web surface, with the event log kept raw."
+++

## Goal

Make niucode's transcript and niuse's companion surface pleasant to read:
emphasis, code, tables, math fallback, clear progress, and eventually a web
client — without turning the event log into presentation data.

## Phases

### P1 · Inline Markdown and tables (in progress)

- `markdown::inline`: bold, italic, inline code, links, strikethrough.
- ATX headings lose their `#` markers.
- `table::render`: buffered pipe tables, borders, alignment, width cap.
- Both linear ANSI and ratatui spans use the same spans.
- Tests: parser edge cases, table widths, PTY visible output.

### P2 · Blocks and polish

- Lists, block quotes, task checkboxes, horizontal rules. **M9-003** covers
  `-`/`*` bullets, `☐`/`☑` tasks, and `>` quotes; ordered lists keep `1.`.
- Table cells reuse inline spans; numeric right alignment.
- Code fences are hidden and replaced by a `▎ language` label.
  **M9-004**.
- Math fallback conventions (`$…$`, `$$…$$`) and copy-friendly output.
  **M9-005** buffers display blocks and strips the `$$` fences.
  **Included in M9-002**: common symbols, `\frac`, `\sqrt`; unmatched `$` is
  literal.
- Link display strategy for terminals without hyperlink support.

### P3 · Motion and progress

- Streaming caret or shimmer that can be disabled. **M9-006** adds a
  `▍`/`▏` caret and a per-tool elapsed timer.
- Per-tool progress with elapsed time and a stable collapsed line.
- Completion transitions that do not reflow history.
- Respect `NO_COLOR`, `TERM=dumb`, and reduced-motion environments.

### P4 · Web surface

- A local HTTP + SSE server over the same agent loop and event log.
- Static, no-build client; Markdown and KaTeX rendering.
- Session list, streaming answer, tool/approval cards, cancel.
- Explicit local-only binding and token; no remote exposure in v1.
- The kernel/product split: niucode owns the protocol and reference client;
  niuse owns identity, memory, and product defaults.

## Invariants

- The event log stays raw text; rendering is derived.
- Model-provided ANSI is scrubbed.
- Width and cursor calculations use terminal cell width, not bytes.
- Every phase ships tests, docs, and a dashboard card.
