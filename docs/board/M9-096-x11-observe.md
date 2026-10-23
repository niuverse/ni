+++
id = "M9-096"
title = "X11 observe backend"
status = "done"
milestone = "M9"
module = "tools"
size = "M"
priority = "P0"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-086"]
docs = ["adr-0037"]
+++

## Problem

The protocol had observation-bound actions but no OS backend to produce a real
window observation or screenshot.

## Scope

- Add `niucode-computer-x11` (Linux-only).
- Pure Rust `x11rb` connection; no xdotool/scrot shelling.
- Enumerate viewable windows with title/class/geometry.
- Capture a window as PNG and derive `Observation` metadata with a 60s expiry.
- Classify terminal and sensitive windows for policy decisions.
- Xvfb-backed integration test, skipped when Xvfb is absent; CI installs it.
- No input injection, no provider/tool wiring yet.

## Acceptance

- [x] Window list includes a mapped test window.
- [x] Capture returns PNG and native dimensions.
- [x] Observation id/hash/expiry are populated.
- [x] Terminal/sensitive classification tests pass.
- [x] Backend compiles only on Linux; macOS CI is unaffected.
- [x] CI installs xvfb for the integration path.

## Notes

X11 has no application-level isolation; these checks are product policy, not an
OS sandbox. Input injection and the tool/approval surface are next.
