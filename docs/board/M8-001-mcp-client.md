+++
id = "M8-001"
title = "MCP stdio client v1"
status = "doing"
milestone = "M8"
module = "extensions"
size = "L"
priority = "P1"
updated = "2026-10-22"
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
- [ ] `exec --mcp-config` and the TUI mount the same host; `doctor` reports it.
- [ ] Offline tests cover mount, list, call, timeout, and shutdown.

## Notes

Client, host, namespaced specs, collision check, and agent dispatch are done
and tested. Remaining: load `~/.niucode/mcp.toml`, mount the host in `exec` and
the TUI, and report mounts in `doctor`. See ADR-0033 and [mcp](../mcp.md).
