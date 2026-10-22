+++
id = "M5-010"
title = "Read-only embedder tools"
status = "done"
milestone = "M5"
module = "loop"
size = "S"
priority = "P1"
updated = "2026-10-22"
completed = "2026-10-22"
depends_on = ["M4-002"]
docs = ["adr-0032", "tui"]
+++

## Problem

An embedder can augment the prompt, observe events, filter text, and register
slash commands, but it cannot expose a read-only tool such as "search my
personal memory". The extension host is session-scoped and workspace-mounted;
an embedder that owns the product data has nowhere to put a bounded query tool
without forking the loop or adding product schema to the kernel.

## Scope

- `niucode_agent::EmbedderTool`: `spec()`, `read_only()` (default true), and an
  async `execute()`.
- `AgentConfig.extra_tools` and `TuiOptions`/`ExecOptions.extra_tools`.
- Advertise specs next to built-ins and the extension host; dispatch by name.
- Reject mutating embedder tools and name collisions before the first turn.
- Do not inherit extra tools into subagents; do not add a write path or an
  approval path for them in v1.

## Acceptance

- [x] A replay turn calls an embedder tool and observes its `ToolResult`.
- [x] The tool spec is part of the advertised surface.
- [x] A non-read-only tool is rejected before the turn.
- [x] A name collision with a built-in is rejected before the turn.
- [x] `cargo test --workspace`, clippy, and the dashboard pass.

## Notes

The seam is intentionally narrower than the extension host: read-only,
in-process, and explicitly not inherited. niuse uses it for `memory_search`
(M6-005); a future mutating or out-of-process tool should extend the extension
host, not this trait.
