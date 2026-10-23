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
- Table cells reuse inline spans; numeric right alignment. **M9-019** folds
  header/border tone into table line metadata; **M9-020** carries cell inline
  styles as byte ranges.
- Code fences are hidden and replaced by a `▎ language` label.
  **M9-004**; **M9-021** adds line-based token highlighting.
- Math fallback conventions (`$…$`, `$$…$$`) and copy-friendly output.
  **M9-005** buffers display blocks and strips the `$$` fences.
  **Included in M9-002**: common symbols, `\frac`, `\sqrt`; unmatched `$` is
  literal.
- Link display strategy for terminals without hyperlink support.

### P3 · Motion and progress

- Streaming caret or shimmer that can be disabled. **M9-006** adds a
  `▍`/`▏` caret and a per-tool elapsed timer.
- Per-tool progress with elapsed time and a stable collapsed line. **M9-027**
  adds one `→ tool` marker per type per turn; failures stay explicit.
- Completion transitions that do not reflow history.
- Respect `NO_COLOR`, `TERM=dumb`, and reduced-motion environments.

### P4 · Web surface

- **v0 (M9-007)**: loopback-only read-only viewer, embedded static client,
  `/api/session`.
- **Live tail (M9-008)**: `/api/events` chunked SSE, initial log + appended
  lines.
- **Read-only chat (M9-009)**: `POST /api/prompt`, one turn at a time, hard-coded
  read-only/never, embedded composer. **M9-031** makes it multi-line.
- **Streaming (M9-010)**: runtime deltas broadcast over SSE and finalized by the
  durable assistant event.
- **Approvals (M9-011)**: browser Allow/Deny releases a blocked approver; previews
  are serialized; unknown IDs 404. **M9-024** renders them as readable diffs.
- **Markdown (M9-012)**: server-rendered HTML with escaped raw HTML, tables, and
  safe links.
- **Math (M9-013)**: native MathML from common TeX, escaped fallback, no remote
  assets. KaTeX remains an optional upgrade.
- **Code (M9-022)**: fenced code keeps its language and token colors in the
  browser via escaped server-rendered HTML. **M9-026** carries multi-line
  block-comment state; **M9-028** keeps triple-quoted strings across lines.
- **Streaming (M9-023)**: each text delta carries accumulated rendered HTML, so
  Markdown forms while the turn is still running.
- **Tools (M9-025)**: collapsible call/result cards with escaped, bounded
  input/output.
- **Context (M9-015)**: embedder brand + `/api/context` + generic sidebar.
  **M9-029** makes sections collapsible/filterable; **M9-030** refreshes them
  after turns and remembers collapsed groups.
- **Status (M9-016)**: model/cwd/policy chips from `/api/status`.
- **Turn timer (M9-017)**: SSE phases drive a busy chip and elapsed timer; the
  context sidebar can be collapsed. See
  [ADR-0036](adr/0036-local-web-surface.md).
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
