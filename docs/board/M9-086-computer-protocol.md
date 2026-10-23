+++
id = "M9-086"
title = "Computer-use protocol foundation"
status = "done"
milestone = "M9"
module = "tools"
size = "S"
priority = "P0"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-085"]
docs = ["adr-0037"]
+++

## Problem

The computer-use design needed executable contracts before any OS backend:
permission tiers, observation binding, action serialization, and red-line
decisions.

## Scope

- Add `niucode-computer` with policy, action, observation, and validation types.
- Encode the four permission tiers and autonomous red lines.
- Validate stale observations, window mismatch, coordinates, text, keys, and
  scroll bounds.
- Add ADR-0037.
- No X11/macOS backend, no CLI flag, no niuse behavior yet.

## Acceptance

- [x] Policy parses off/observe/control/autonomous and defaults off.
- [x] Permission matrix tests cover observe/input/autonomous red lines.
- [x] Actions serialize with an `action` tag and image-relative coordinates.
- [x] Stale/expired observations, wrong windows, and out-of-bounds points reject.
- [x] Empty text/key and invalid scroll reject.
- [x] Crate has no OS dependency and compiles on all targets.

## Notes

The next slice adds provider image parts and attachment storage; X11 observe
follows after context projection.
