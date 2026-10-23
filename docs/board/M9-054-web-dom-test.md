+++
id = "M9-054"
title = "DOM id consistency test for the web client"
status = "done"
milestone = "M9"
module = "web"
size = "S"
priority = "P1"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-053"]
docs = ["adr-0036"]
+++

## Problem

The embedded client is a single HTML file. A typo in an `id` or a
`getElementById` call would silently break a control, and no test noticed.

## Scope

- Parse `id="…"` values and `getElementById('…')` references from the embedded
  client.
- Assert IDs are unique and every referenced ID exists.
- Keep the existing no-remote-assets assertion.
- No runtime change.

## Acceptance

- [x] All current IDs are unique.
- [x] Every `getElementById` reference resolves.
- [x] Test runs offline in the Rust suite.
- [x] Embedded script parses with `node --check` in the audit.
- [x] Existing static/JS tests pass.

## Notes

This is a structural canary, not a DOM runtime test. A headless-browser smoke
test remains a future CI upgrade.
