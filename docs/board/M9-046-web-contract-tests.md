+++
id = "M9-046"
title = "HTTP contract coverage for export and new session"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-040", "M9-043"]
docs = ["adr-0036"]
+++

## Problem

Export and new-session behavior had unit coverage but their HTTP contracts
(status, content type, disposition, disabled handling) were untested.

## Scope

- Add loopback tests for:
  - `GET /api/export` → 200, `text/markdown`, attachment filename, body.
  - `POST /api/new` with `allow_new: false` → 403.
- Keep existing create/allow tests.
- No production behavior change unless a test exposes a defect.

## Acceptance

- [x] Export response carries the Markdown content type and disposition.
- [x] Exported body includes the session title and user text.
- [x] Disabled new-session returns 403.
- [x] Tests use the same state helper and loopback server.
- [x] Full web test suite passes.

## Notes

These are protocol-level tests; browser behavior remains covered by static client
assertions.
