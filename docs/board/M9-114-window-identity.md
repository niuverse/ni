+++
id = "M9-114"
title = "Window identity recheck before input"
status = "done"
milestone = "M9"
module = "tools"
size = "S"
priority = "P0"
updated = "2026-10-24"
completed = "2026-10-24"
depends_on = ["M9-097", "M9-102"]
docs = ["adr-0037"]
+++

## Problem

Input dispatch rechecked only window id + bounds. A window could keep the same
id/geometry while its app/class or sensitive/terminal classification changed,
letting a stale observation authorize input against a different surface.

## Scope

- Add `WindowInfo::same_identity`: id, app, class, and kind must match. Titles
  are deliberately excluded because typing changes them.
- Use it in both X11 and macOS before input dispatch.
- Xvfb regression moves the window after capture and asserts `StaleWindow`.
- Unit test proves title changes are allowed while app/kind changes are not.

## Acceptance

- [x] X11 rejects moved-window clicks.
- [x] macOS uses the same identity predicate.
- [x] Title-only changes remain legitimate.
- [x] App/class/kind changes reject.
- [x] Tests/clippy pass.

## Notes

Identity does not compare screenshot content; a same-window visual change within
the 60s expiry is still allowed. Observation expiry remains the time bound.
