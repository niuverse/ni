+++
id = "M8-002"
title = "Mutating MCP with per-call approval"
status = "ready"
milestone = "M8"
module = "mcp"
size = "L"
priority = "P1"
updated = "2026-10-23"
depends_on = ["M8-001"]
docs = ["adr-0034", "mcp"]
+++

## Problem

Read-only packs cannot create a note or update a task. A write path must not
become the least-guarded surface in the harness.

## Scope

- Manifest `write_tools = [...]` allowlist and `--allow-mcp-write`.
- Refuse write-enabled servers in `never`/`full-access`/non-interactive runs.
- Per-call approval with tool, server, and bounded argument preview.
- Shared write lock, no retry on timeout, existing approval/tool_result events.
- No subagent inheritance.

## Acceptance

- [ ] A write tool not in `write_tools` is not exposed as mutating.
- [ ] A listed-but-missing tool fails the mount.
- [ ] `--allow-mcp-write` without an interactive approval mode refuses to mount.
- [ ] Approval shows the namespaced tool and arguments; denial writes no change.
- [ ] A timed-out mutation is not retried automatically.
- [ ] Subagents never see mutating MCP tools.
- [ ] Offline tests cover allowlist, refusal, deny, timeout, and no-retry.

## Notes

ADR-0034 was accepted by the human on 2026-10-23. The design is now settled:
manifest allowlist, `--allow-mcp-write`, interactive approval only, args
preview, shared write lock, no retry, no subagent inheritance. Implementation
may proceed.
