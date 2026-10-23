+++
id = "M9-070"
title = "Mobile overlay dismissal and focus"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-069"]
docs = ["adr-0036"]
+++

## Problem

The mobile context overlay had no backdrop, no Escape handling, and moved focus
nowhere. Tapping outside it did nothing.

## Scope

- Add a narrow-screen backdrop below the header.
- Backdrop click closes the overlay.
- `Escape` closes it.
- Opening focuses the context filter; closing focuses the toggle.
- Keep the backdrop hidden by default and on desktop.
- No data/API change.

## Acceptance

- [x] Backdrop appears only on narrow screens while open.
- [x] Backdrop click and Escape close it.
- [x] Focus moves to filter on open and toggle on close.
- [x] Desktop layout is unaffected.
- [x] Static assertion covers the backdrop.
- [x] 390px screenshot confirms the overlay/backdrop.

## Notes

Focus is moved but not trapped; a full modal focus trap is a later a11y slice.
