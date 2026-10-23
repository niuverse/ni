+++
id = "mcp"
title = "MCP stdio client"
kind = "doc"
status = "living"
order = 70
summary = "MCP stdio: read-only by default, opt-in writes with an allowlist and per-call approval, manifest shape, and limits."
+++

## Status

The `niucode-mcp` crate implements the v1 client and host from
[ADR-0033](adr/0033-mcp-client.md): spawn, `initialize`, `tools/list`,
`tools/call`, and a multi-server host over newline-delimited JSON on stdio.
`AgentConfig.mcp_host` advertises namespaced specs and dispatches them in the
turn loop. It is covered offline by a Python fixture server, including list,
call, tool error, environment scrubbing, timeout, host mounting, collisions,
and an agent turn.

`exec` and the TUI load `~/.niucode/mcp.toml` (or `--mcp-config PATH`), mount
read-only servers, print one `mcp: <server> · <n> tool(s)` line per mount, and
shut the children down at the end of the run. The host is shared by both paths.

## Manifest

`~/.niucode/mcp.toml` is explicit trust. A workspace file is never read.

```toml
[[server]]
name = "notes"
command = "python3"
args = ["/home/me/notes-server.py"]
read_only = true

[server.env]
NOTES_DIR = "/home/me/notes"
```

Only `server.env` is passed to the child. `read_only = false` is refused unless
the manifest names every mutating tool and the run passes `--allow-mcp-write`
with an interactive approval mode (`ask` or `on-failure`). Unnamed tools from a
write server stay read-only; a named tool the server does not expose fails the
mount. `--json` and `--approval never` mount read-only servers only.

```toml
[[server]]
name = "notes"
command = "python3"
args = ["/home/me/notes-server.py"]
read_only = false
write_tools = ["create_note", "update_note"]
```

Every mutating call is approved with the namespaced tool, server, and a bounded
JSON argument preview. The prompt says the effect is server-defined; denial
writes no change. A timeout is not retried [[ADR-0034](adr/0034-mutating-mcp.md)].

## Rules

- mutating calls take the shared write lock and are never inherited by subagents.
- stdio only; no HTTP/SSE.
- tools only; no resources, prompts, sampling, or elicitation.
- namespaced as `mcp__<server>__<tool>`; collisions are startup errors.
- stdout is protocol, stderr is diagnostics; a child that hangs is killed at
  the timeout.
- the child is an ordinary local process running as the user. The manifest is a
  trust decision, not a sandbox.

## Verify

```sh
cargo test -p niucode-mcp
cargo test -p niucode --test mcp
niucode exec --mcp-config /path/to/mcp.toml "list the notes"
```
