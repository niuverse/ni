+++
id = "M8-001"
title = "MCP stdio client v1"
status = "done"
milestone = "M8"
module = "extensions"
size = "L"
priority = "P1"
updated = "2026-10-22"
completed = "2026-10-22"
depends_on = ["M4-002"]
docs = ["adr-0033", "exec", "tui"]
+++

## Problem

Every external capability currently needs a bespoke extension manifest and
sidecar. Personal services increasingly speak MCP; without a client, niucode
cannot reuse that ecosystem and niuse cannot reach personal data without a new
integration each time.

## Scope

- Read `~/.niucode/mcp.toml` (explicit `read_only = true` servers only).
- stdio transport: `initialize`, `tools/list`, `tools/call`, timeouts, process
  group shutdown.
- Expose tools as `mcp__<server>__<tool>`; reject collisions.
- Scrubbed environment; only manifest-declared variables.
- Mount/unmount and call audit events; no new durable store.
- Do not inherit MCP tools into subagents in v1.

## Acceptance

- [x] A Python stdio MCP server mounts, lists one tool, and is callable.
- [x] A non-read-only manifest is refused before any turn.
- [x] A name collision is refused before any turn.
- [x] Credentials from the parent environment do not reach the server.
- [x] A hung server times out and is killed without stalling the session.
- [x] `exec --mcp-config` and the TUI mount through the same host; startup prints each mount.
- [x] Offline tests cover mount, list, call, timeout, shutdown, and an exec turn.

## Notes

Client, host, namespaced specs, collision check, agent dispatch, config loading,
and `exec`/TUI mounting are done and tested. Startup prints each mount. A
dedicated `doctor` does not exist in niucode; the TUI-specific PTY mount test is
still a follow-up. See ADR-0033 and [mcp](../mcp.md).
