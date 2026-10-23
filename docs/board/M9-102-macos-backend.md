+++
id = "M9-102"
title = "macOS native computer backend"
status = "done"
milestone = "M9"
module = "tools"
size = "L"
priority = "P0"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-101"]
docs = ["adr-0037"]
+++

## Problem

Computer use was Linux-only; the second product platform had no window
enumeration, capture, or input.

## Scope

- Add `niucode-computer-macos` (macOS-only).
- CoreGraphics window list: id, app, title, bounds.
- Window snapshot to PNG; observation hash/expiry/scale.
- CGEvent click, Unicode text, named keys/modifiers, scroll.
- Screen Recording preflight at connect; Accessibility checked before input.
- Emergency stop releases modifiers and mouse buttons.
- Wire `--computer` through the shared CLI configuration on macOS.
- Unit tests for classification/key mapping; cross-target `cargo check/clippy`
  locally and macOS CI compilation.

## Acceptance

- [x] Window enumeration and PNG capture compile against macOS SDK types.
- [x] Input is observation-bound and geometry-checked.
- [x] TCC checks fail closed with actionable errors.
- [x] Emergency stop is implemented.
- [x] Linux/macOS target cfg separation stays clean.
- [x] Tests/clippy pass; macOS CI is the runtime gate.

## Notes

The first slice uses CoreGraphics `CGWindowListCreateImage`; ScreenCaptureKit is
a later upgrade. TCC cannot be granted programmatically. Actual macOS runtime
interaction still needs a signed, permission-granted dogfood run.
