+++
id = "adr-0033"
title = "ADR-0033: MCP client v1 — stdio, explicit trust, read-only tools"
kind = "adr"
status = "accepted"
order = 60
summary = "A minimal MCP client: stdio servers from an explicit trust file, tools only, read-only in v1, with namespaced tools, audit events, timeouts, and no inherited credentials."
+++

## Status

Accepted · 2026-10-22. This is the first M8 (Connected harness) decision. It
does not change the event log or the extension-host trust model.

## Context

The extension host (ADR-0013) is a session-scoped, trusted local sidecar with a
custom line-delimited JSON-RPC protocol. It solved capability-level external
tools without kernel forks, but every integration writes a bespoke manifest and
sidecar. MCP is the common protocol for tools and resources; a personal agent
needs it to reach notes, calendars, and project services without a new
integration per service.

Two things make a naive client dangerous:

- an MCP server is an arbitrary local process that can read the environment,
  the filesystem, and the network;
- an MCP tool executes outside niucode's built-in approval and write-lock path.

The current extension host already accepts mutating tools without an approval
prompt; repeating that for MCP would widen the least-bounded surface in the
harness.

## Decision

1. **stdio only in v1.** No HTTP/SSE transport, no remote URL, no OAuth. A
   server is a local command with argv and cwd.
2. **Explicit trust, never auto-discovery.** Servers live in
   `~/.niucode/mcp.toml` with `name`, `command`, `args`, optional `env`, and
   `read_only = true`. A workspace `.niucode/mcp.toml` is not read in v1.
   Nothing starts unless the user wrote it down.
3. **Tools only.** Resources, prompts, sampling, and elicitation are out of
   scope. They need their own permission model.
4. **Read-only in v1.** A manifest must declare `read_only = true`; the client
   rejects a server whose tools are not read-only until a mutating proposal
   with approval, preview, and a shared write lock exists.
5. **Namespaced and collision-checked.** Exposed names are
   `mcp__<server>__<tool>`; a collision with a built-in, embedder tool, or
   another server is a startup error.
6. **No inherited credentials.** Spawn with a scrubbed environment; only
   variables named in the manifest are passed. The server does not receive
   provider keys, auth tokens, or `NIUCODE_*` state.
7. **Bounded and observable.** Initialize, list, and call have timeouts; the
   process group is killed on shutdown; stdout is parsed as line-delimited
   JSON and stderr is logged, never treated as protocol. Mount and call
   produce events so replay can explain where a tool came from.
8. **One tool budget.** A result is capped by the same tool-output budget as
   built-ins and enters the conversation only through the loop's normal
   transcript projection.

## Consequences

**Better**

- One protocol for many personal services, without a new manifest format per
  service.
- The trust decision is visible in one file and one ADR.
- Read-only v1 cannot mutate the user's data or bypass approvals.

**Worse**

- Read-only limits usefulness until a mutating design lands.
- stdio-only excludes hosted MCP servers; that is deliberate for v1.
- Each server adds a process to supervise and a failure mode to report.

## Alternatives

- **HTTP/SSE first.** Rejected: more auth, more network surface, no personal
  need yet.
- **Import MCP tools through the extension host.** Rejected: the extension
  protocol and MCP have different lifecycles; a separate client keeps both
  contracts honest.
- **Allow mutating tools with the existing extension host behaviour.**
  Rejected: it has no approval story and would make MCP the least-guarded
  write path.
- **Auto-discover servers from the workspace.** Rejected: a cloned repository
  must not be able to start a local process.

## Caveat

MCP servers are still arbitrary code running as the user. The manifest is a
trust decision, not a sandbox. v1 is for servers the user chose and reviewed;
network and filesystem confinement remain an M4-007-class capability gate.
