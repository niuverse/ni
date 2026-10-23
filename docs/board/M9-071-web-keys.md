+++
id = "M9-071"
title = "Web keyboard shortcuts and overlay focus trap"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-070"]
docs = ["adr-0036"]
+++

## Problem

Keyboard users could tab through the page but had no direct way to reach search
or the composer, and the mobile context overlay did not trap focus.

## Scope

- `Ctrl/Cmd+K` focuses transcript search.
- `Ctrl/Cmd+/` focuses the composer.
- `Escape` priority: close context overlay → clear focused transcript search →
  close command menu.
- `Tab`/`Shift+Tab` cycles inside the open mobile overlay.
- No server/API change.
- Static assertions cover the handlers.

## Acceptance

- [x] Ctrl/Cmd+K and Ctrl/Cmd+/ focus the right controls.
- [x] Escape closes overlay first, then clears search, then closes the menu.
- [x] Tab wraps within the overlay while open.
- [x] Desktop shortcuts work without opening the overlay.
- [x] Existing composer/command tests pass.

## Notes

The command menu's own arrow/Tab handling still wins while it is open. A future
slice can add a visible shortcut cheat-sheet.
