+++
id = "M9-123"
title = "Expand system-shortcut red lines"
status = "done"
milestone = "M9"
module = "tools"
size = "S"
priority = "P1"
updated = "2026-10-24"
completed = "2026-10-24"
depends_on = ["M9-086"]
docs = ["adr-0037"]
+++

## Problem

Autonomous mode treated Alt+Tab, Alt+Escape, Ctrl+Alt+Backspace, and Ctrl+Alt+F1
.. F12 as ordinary keys. Those can switch context, kill the X server, or change
virtual terminals, escaping the observed window.

## Scope

- Mark these as system shortcuts:
  - Super + anything (already)
  - Alt+F4 (already)
  - Alt+Tab
  - Alt+Escape
  - Ctrl+Alt+Delete (already)
  - Ctrl+Alt+Backspace
  - Ctrl+Alt+F1..F12
- Keep Ctrl+C and ordinary Ctrl+Alt+letter shortcuts ordinary.
- Regression covers positive and negative cases.

## Acceptance

- [x] Alt+Tab/Alt+Escape require approval in autonomous.
- [x] VT-switch and X-server shortcuts require approval.
- [x] Ctrl+C and Ctrl+Alt+letter stay ordinary.
- [x] Tests/clippy pass.

## Notes

This is a heuristic red-line list, not a complete OS shortcut database. Control
mode already asks for every input.
