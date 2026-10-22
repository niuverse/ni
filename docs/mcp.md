+++
id = "mcp"
title = "MCP stdio client"
kind = "doc"
status = "living"
order = 70
summary = "Read-only MCP v1: the client crate, manifest shape, security rules, and what is still unmounted."
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

Only `server.env` is passed to the child. `read_only = false` is refused before
spawn; v1 has no mutating path.

## Rules

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
