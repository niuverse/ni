+++
id = "M9-100"
title = "TUI computer entry and stop"
status = "done"
milestone = "M9"
module = "tui"
size = "S"
priority = "P0"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-099"]
docs = ["adr-0037"]
+++

## Problem

Computer use was only reachable from one-shot exec; the interactive TUI had no
policy entry, status, or way to stop it.

## Scope

- Add `niucode tui --computer off|observe|control|autonomous`.
- Share the X11 runtime/arm setup with exec.
- Add `/computer`, `/computer arm`, `/computer stop`.
- `stop` sets policy off and disarms; in-flight actions finish, future actions
  are denied.
- PTY check for the off/unavailable status; CLI parsing test.

## Acceptance

- [x] TUI accepts `--computer` and arms a Linux X11 session.
- [x] `/computer` reports policy and armed state.
- [x] `/computer stop` disarms and turns policy off.
- [x] `/computer arm` re-arms the current policy.
- [x] Off sessions remain zero-backend.
- [x] Existing TUI attachment PTY test passes.
- [x] Tests/clippy pass.

## Notes

A process-wide emergency hotkey is still missing; `/computer stop` is immediate
but requires the composer. The next slice owns key-release/global stop.
