+++
id = "M9-052"
title = "High-contrast web theme"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-045"]
docs = ["rendering", "adr-0036"]
+++

## Problem

Dark and light modes were both low-contrast surfaces. Users who need stronger
separation had no option.

## Scope

- Add a `body.contrast` variable set: black surfaces, white text/borders, yellow
  accent, strong success/error.
- Override diff and code-token colors for contrast.
- Cycle the theme button dark → light → high contrast → dark; persist the mode.
- Keep reduced-motion behavior unchanged.
- No server or session change.

## Acceptance

- [x] Contrast variables override core colors.
- [x] Theme cycles through three modes and persists.
- [x] Diff/token colors have contrast overrides.
- [x] Button label reflects the mode.
- [x] Static client assertion covers `body.contrast`.
- [x] Existing theme/reduced-motion tests pass.

## Notes

Contrast is a presentation preference. A future slice can honor
`prefers-contrast: more` automatically.
