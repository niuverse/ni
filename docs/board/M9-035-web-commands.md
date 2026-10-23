+++
id = "M9-035"
title = "Local slash commands in the web composer"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-031"]
docs = ["rendering", "adr-0036"]
+++

## Problem

Every entered line was sent to the model. The TUI has local slash commands, but
the browser had no way to inspect status or jump to the report without spending
a turn.

## Scope

- `/help` lists the local commands.
- `/status` prints model, cwd, sandbox/approval, and network from `/api/status`.
- `/report` clears the context filter, reloads context, and filters to the
  report section.
- Commands are handled in the client and never POSTed.
- Commands remain in local prompt history; nothing is written to the session.
- Unknown `/…` still goes to the agent (for now).

## Acceptance

- [x] `/help`, `/status`, and `/report` do not call `/api/prompt`.
- [x] Status comes from the existing endpoint.
- [x] Report uses the existing context/sidebar path.
- [x] Input and height reset after local commands.
- [x] Static client assertions cover `/help`.
- [x] Existing prompt/approval tests pass.

## Notes

Unknown slash commands are intentionally not intercepted; a later slice can add
strict command routing and `/clear` semantics tied to a new session.
