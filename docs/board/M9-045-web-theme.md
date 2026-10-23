+++
id = "M9-045"
title = "Light theme and reduced motion in web"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-044"]
docs = ["rendering", "adr-0036"]
+++

## Problem

The web shell was dark-only. A user who needs a light surface or reduced motion
had no control, and smooth scroll ignored the OS preference.

## Scope

- Promote core colors to CSS variables with a `body.light` override.
- Add a ☾/☀ header toggle persisted in `localStorage`.
- Override diff/token/approval colors for light mode.
- Add `prefers-reduced-motion` CSS and make jump-to-latest use `auto` when
  reduced motion is requested.
- No server, store, or event changes.

## Acceptance

- [x] Light variables cover background, panel, text, border, code.
- [x] Toggle persists across reloads.
- [x] Light-mode diff/token overrides remain readable.
- [x] Reduced-motion media query exists.
- [x] Latest scroll honors reduced motion.
- [x] Static client assertions cover theme and motion.

## Notes

The theme is a presentation preference only; it never touches the transcript or
session. A future slice can add a high-contrast variant.
