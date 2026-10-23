+++
id = "M9-037"
title = "Slash-command menu in the web composer"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-035"]
docs = ["rendering", "adr-0036"]
+++

## Problem

The local commands were only discoverable through `/help`; typing a slash gave
no feedback.

## Scope

- Show a menu above the composer while the input starts with `/`.
- Filter by prefix; show name and description.
- `Tab` or click fills the command; `↑`/`↓` move the active item.
- Hide the menu for spaces, empty prefixes, or no matches.
- No server route; commands still execute through the local handler.

## Acceptance

- [x] `/` opens a menu of known commands.
- [x] Prefix filtering works.
- [x] Tab/click fills the command without sending it.
- [x] Arrow keys cycle items.
- [x] Menu is hidden for non-command input.
- [x] Static client assertions cover `cmd-menu`.

## Notes

Full keyboard handling of history vs menu when both are active is intentionally
simple; the menu wins only for Tab/arrows when visible.
