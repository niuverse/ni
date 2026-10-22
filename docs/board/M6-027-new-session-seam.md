+++
id = "M6-027"
title = "New-session path seam for embedders"
status = "done"
milestone = "M6"
module = "tui"
size = "M"
priority = "P1"
updated = "2026-10-22"
completed = "2026-10-22"
depends_on = ["M6-026"]
docs = ["tui", "adr-0032"]
+++

## Problem

`/clear` and `/reset` created a timestamped session in the session directory.
An embedder that owns its own thread file (niuse uses `thread.jsonl`) was left
pointing at the old log, so product commands read and wrote the wrong thread.

## Scope

- `SessionLog::create_at(path)`: create at an embedder path, never overwrite.
- `TurnHooks::new_session_path()`: embedder archives the old log and returns the
  new path; `None` keeps the timestamped default.
- `TurnHooks::on_session_start(path)`: embedder updates path-sensitive state.
- Wire both `/clear` and `/reset` through the seam.
- Tests for create_at, niuse archive/pointer, and PTY isolation.

## Acceptance

- [x] `create_at` uses the chosen path and refuses an existing file.
- [x] `/clear` and `/reset` call the hook before creating the new session.
- [x] The hook is notified with the new path.
- [x] niuse archives the old thread, returns `thread.jsonl`, and updates hooks.
- [x] PTY: `/new` archives FIRST_ANSWER, active thread has only SECOND_ANSWER.
- [x] No-overwrite is preserved for the default TUI path.

## Notes

This is a generic seam, not a niuse special case. The default remains a
timestamped file; only an embedder that supplies a path changes behaviour.
