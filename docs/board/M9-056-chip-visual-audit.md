+++
id = "M9-056"
title = "Visual audit fixes blank header chip buttons"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-055"]
docs = ["adr-0036"]
+++

## Problem

A real headless-Chrome render showed the theme, download, and context-toggle
buttons as blank blue pills. The global `button` rule set a blue background and
small dark text; `.chip` only changed the border, so the labels were effectively
invisible.

## Scope

- Add `button.chip` overrides: panel background, dim text, hover accent border.
- Keep the global primary button style for Send/Allow.
- Add a static client assertion for `button.chip`.
- Capture dark and light screenshots through headless Chrome as audit evidence.

## Acceptance

- [x] Header chip buttons are readable in light and dark themes.
- [x] Send remains the primary button.
- [x] Static assertion covers the override.
- [x] Headless smoke test and existing web tests pass.
- [x] No layout or session change.

## Notes

This is the first visual regression caught by the headless path. Screenshot diffing
in CI remains future work.
