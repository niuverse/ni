+++
id = "adr-0034"
title = "ADR-0034: Mutating MCP — explicit tool allowlist and per-call approval"
kind = "adr"
status = "accepted"
order = 61
summary = "Mutating MCP stays off by default. A write-enabled server names each write tool, mounts only with an explicit flag and an interactive approval mode, and every call is approved with its arguments and recorded."
+++

## Status

Accepted · 2026-10-23 (human sign-off). Implementation starts in M8-002. Until
the first write tool ships, the client keeps the ADR-0033 rule in production:
only `read_only = true` servers mount, and `read_only = false` is refused
before spawn.

## Context

Read-only MCP is shipped and tested. The next useful personal pack (create a
note, add a calendar entry) needs a write. The existing write paths have three
properties MCP currently lacks:

- a human sees the proposed change before it lands;
- built-in writes share a workspace write lock;
- the result is auditable in the event log.

An MCP server is an arbitrary local process. niucode cannot inspect a generic
`write_note` argument and produce a trustworthy diff, cannot confine a process
that ignores its declared roots, and cannot undo a remote side effect. The
design must therefore be explicit about what approval means: the human trusts
the server and approves the exact tool plus arguments, not a filesystem diff.

## Decision

1. **No write by default.** A server with `read_only = false` is refused unless
   both conditions hold: the manifest names the write tools in
   `write_tools = [...]`, and the run passes `--allow-mcp-write`.
2. **Per-tool allowlist, never `write = true`.** Every mutating tool must be
   named. A listed tool that the server does not expose is a mount error, not a
   silent omission. Unlisted tools remain read-only.
3. **Interactive approval only.** A mutating call requires an approval mode
   with a human in the loop (`ask` or `review`). `never`, `full-access`,
   `--json`, and other non-interactive paths do not mount write-enabled
   servers. "Always allow" is not offered in v1.
4. **Approval shows the call, not a fake diff.** The prompt contains the
   namespaced tool, the server, and a bounded JSON argument preview. The text
   says the effect is server-defined. Denial is a tool error and is recorded.
5. **Shared lock, honest limits.** A mutating call takes the same write lock as
   built-in writes so the agent cannot interleave its own edits. The lock is
   serialization, not a sandbox: the server still runs as the user and may
   touch anything it can. The manifest is the trust boundary.
6. **No retry on timeout.** A timed-out mutation has an unknown outcome. The
   client kills the process group, records the failure, and tells the model not
   to retry without human confirmation.
7. **Bounded result and arguments.** Argument preview and tool output use the
   existing tool-output budget. Secrets are not redacted automatically; the
   manifest must not put credentials in arguments the agent can read.
8. **Audit.** Every mutating call writes the existing `approval` event
   (allowed/denied) and `tool_result`; a mount writes its config hash and the
   write-tool list. No new durable store.
9. **No subagent inheritance.** A subagent never receives mutating MCP tools.
   A future worker profile needs its own approval design.
10. **stdio only.** No HTTP transport or remote OAuth in this slice.

## Consequences

**Better**

- A write cannot happen without a human seeing the tool and arguments.
- `read_only = false` alone cannot silently widen the surface.
- The audit trail uses events that already exist.

**Worse**

- Approval quality depends on the arguments being legible; a tool that hides
  its effect behind a blob is dangerous even when approved.
- No automatic rollback, diff, or filesystem confinement.
- Write-enabled servers are useless in headless/JSON runs by design.

## Alternatives

- **Treat all server tools as read-only and rely on the server.** Rejected:
  names lie; the harness would claim a guarantee it cannot enforce.
- **Require a dry-run/diff tool before apply.** Attractive, but not part of
  MCP and not implementable for arbitrary servers. Revisit if a pack adopts a
  `preview` convention.
- **Run servers in a container.** Stronger, but a separate sandbox project;
  does not replace approval.
- **Auto-approve in `full-access`.** Rejected: `full-access` is about the
  workspace, not an arbitrary external process.

## Open questions for the human

- Is per-call approval enough, or should a write-enabled server also require a
  one-time session confirmation at mount?
- Should a denied call be visible in `niuse evidence` as `denied` even when the
  model retries?
- Do we need a manifest `roots` field for documentation, knowing it is not
  enforced?
