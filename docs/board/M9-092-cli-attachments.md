+++
id = "M9-092"
title = "CLI attachment flags"
status = "done"
milestone = "M9"
module = "cli"
size = "M"
priority = "P0"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-091"]
docs = ["adr-0037"]
+++

## Problem

Image and script support existed in the runtime but no product entry point could
actually attach a file.

## Scope

- Add repeatable `--attach` (text) and `--image` to `niucode exec`.
- Ingest into the session's `attachments/` store and verify the flag kind.
- Reject attachments with `--replay`.
- Pass refs/store through `run_actor_once` to `run_with_attachments`.
- Preserve the existing text-only path and goal follow-up turns.
- Add CLI parsing and ingestion tests.

## Acceptance

- [x] `--attach`/`--image` parse and repeat.
- [x] Text flag rejects images and vice versa.
- [x] Ingested refs reach the agent run.
- [x] New sessions and resumes both work.
- [x] Goal follow-up turns remain text-only.
- [x] Workspace tests/clippy pass.

## Notes

TUI/Web composers and niuse's CLI entry are the next slices. Budget errors
surface from the agent before the user event is recorded.
