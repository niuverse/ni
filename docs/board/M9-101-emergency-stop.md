+++
id = "M9-101"
title = "Computer emergency stop and input release"
status = "done"
milestone = "M9"
module = "tools"
size = "S"
priority = "P0"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-100"]
docs = ["adr-0037"]
+++

## Problem

Stopping computer use relied on setting policy off; held modifiers/buttons were
not released, and Ctrl-C only cancelled the turn without stopping the computer
session.

## Scope

- Add `ComputerBackend::emergency_stop` (default no-op).
- X11 releases Shift/Ctrl/Alt/Super and mouse buttons 1–7 via XTEST.
- Add `ComputerRuntime::emergency_stop`: policy off + disarm + backend release.
- Route exec/TUI Ctrl-C and `/computer stop` through it.
- Keep the existing turn cancellation semantics (exit 130 in exec).
- Tests for policy/disarm/release flag and an Xvfb release call.

## Acceptance

- [x] Emergency stop always turns policy off and disarms.
- [x] X11 releases common modifiers and buttons; repeated calls are safe.
- [x] Ctrl-C in exec and both TUI paths triggers it.
- [x] `/computer stop` uses the same path.
- [x] Release failures are reported but never block disarming.
- [x] Tests/clippy pass.

## Notes

The hotkey is Ctrl-C, not a configurable global shortcut. XTEST calls are
synchronous, so there is no mid-call interruption; release runs immediately
after control returns.
