+++
id = "M8-002"
title = "Mutating MCP with per-call approval"
status = "done"
milestone = "M8"
module = "mcp"
size = "L"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
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

- [x] A write tool not in `write_tools` is not exposed as mutating.
- [x] A listed-but-missing tool fails the mount.
- [x] `--allow-mcp-write` without an interactive approval mode refuses to mount.
- [x] Approval shows the namespaced tool and arguments; denial writes no change.
- [x] A timed-out mutation is not retried automatically (message test).
- [x] Subagents never see mutating MCP tools (explicit `mcp_host = None` and test).
- [x] Offline tests cover allowlist, refusal, deny, timeout, and no-retry.

## Notes

ADR-0034 was accepted by the human on 2026-10-23. The first slice is in:
`write_tools`, `--allow-mcp-write`, mount gating, mutating metadata, per-call
approval, shared write lock, denial, and a no-retry timeout message. Remaining:
a write fixture/pack, a subagent-inheritance test, and the config-hash audit
event.

2026-10-23: the subagent test and the `mcp_mount` event (server, 16-hex config
hash, write-tool list) landed. The hash covers command, args, read_only,
write_tools, and env keys; env values are never recorded. The card is done.
