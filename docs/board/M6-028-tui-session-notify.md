+++
id = "M6-028"
title = "Notify hooks on resume and fork"
status = "done"
milestone = "M6"
module = "tui"
size = "S"
priority = "P1"
updated = "2026-10-22"
completed = "2026-10-22"
depends_on = ["M6-027"]
docs = ["tui", "adr-0032"]
+++

## Problem

`/clear` and `/reset` used the new-session hook, but `/resume` and `/fork`
replaced the session pointer without notifying the embedder. niuse's hooks would
keep reading the old thread after either command.

## Scope

- Central `notify_session` helper around `on_session_start`.
- Call it after `/resume` and `/fork`, and replace the duplicated clear/reset
  calls.
- Unit test with a recording hook.

## Acceptance

- [x] `/resume` notifies hooks after a successful switch.
- [x] `/fork` notifies hooks after opening the destination.
- [x] `/clear` and `/reset` use the same helper.
- [x] A recording hook sees the new path.
- [x] No change to the session files themselves.

## Notes

This fixes the in-session pointer. Whether a resumed or forked session should
become `thread.jsonl` across restarts is a separate product decision.
