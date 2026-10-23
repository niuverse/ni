+++
id = "M9-055"
title = "Headless browser smoke test"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-054"]
docs = ["adr-0036"]
+++

## Problem

DOM id checks and `node --check` prove structure and syntax, not that the single
file actually loads in a browser.

## Scope

- Optional integration test using `CHROME` or `google-chrome`.
- Stub `EventSource` so no server is needed; fetch failures are already caught.
- Load the file over `file://`, dump the DOM with a virtual time budget, and
  assert the log role, brand, and composer are present.
- Skip when Chrome is unavailable; kill after 20s.
- No new dependency or required CI browser.

## Acceptance

- [x] Test passes locally with Chrome.
- [x] Test skips cleanly without Chrome.
- [x] No server or network asset is required.
- [x] Assertions cover transcript role, brand, and prompt.
- [x] Existing Rust/static tests pass.

## Notes

A future CI image can add Chrome and make this non-optional. Screenshot
regression is still out of scope.
