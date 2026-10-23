+++
id = "M9-069"
title = "Mobile context sidebar toggle"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-029"]
docs = ["adr-0036"]
+++

## Problem

Below 980px the sidebar is intentionally hidden by a media query. The header
toggle only changed the `hidden` class, which could not override that rule, so
the context was unreachable on mobile.

## Scope

- Add `aside.open` overlay styling for narrow screens.
- Toggle based on computed visibility instead of class presence.
- First tap opens the overlay; second tap closes it.
- Desktop still hides/shows normally.
- No data or API change.

## Acceptance

- [x] Mobile first tap opens the context overlay.
- [x] Second tap closes it.
- [x] Desktop toggle still works.
- [x] Static assertions cover `aside.open` and computed-style toggle.
- [x] 390px screenshot shows the overlay.
- [x] Existing context/filter tests pass.

## Notes

The overlay has no backdrop; tapping the toggle again closes it. A future slice
can add focus trapping and Escape-to-close.
