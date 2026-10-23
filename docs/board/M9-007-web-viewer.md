+++
id = "M9-007"
title = "Read-only local web viewer"
status = "done"
milestone = "M9"
module = "web"
size = "M"
priority = "P0"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-006"]
docs = ["rendering", "adr-0036"]
+++

## Problem

A terminal is the only way to inspect a session. Long Markdown, tables, and
math would read better in a browser, but a web surface must not become a remote
control plane or a second store.

## Scope

- `niucode web --session <jsonl> [--port N]`, binding loopback only.
- Embedded static client, no build step and no remote assets.
- Endpoints: `/`, `/api/session`, `/api/health`, and a 501 `/api/prompt`.
- Read the append-only log into a snapshot; never mutate it.
- HTTP + loopback test and static-asset test.

## Acceptance

- [x] Server binds `127.0.0.1` and prints the URL.
- [x] `/api/session` returns the loaded events.
- [x] `/` is an embedded HTML client with no `http://`/`https://` assets.
- [x] `/api/prompt` is explicitly unimplemented in v0.
- [x] A loopback HTTP test reads the JSON body.
- [x] Unknown routes return 404 with no stack trace.

## Notes

Chat, SSE, approvals, and KaTeX are follow-ups. This is intentionally a viewer:
it proves transport, embedding, and the no-mutation boundary before adding
interaction.
