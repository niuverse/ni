+++
id = "M9-112"
title = "Computer backend doctor"
status = "done"
milestone = "M9"
module = "cli"
size = "S"
priority = "P1"
updated = "2026-10-24"
completed = "2026-10-24"
depends_on = ["M9-096", "M9-102"]
docs = ["adr-0037"]
+++

## Problem

A user could not tell whether a missing `--computer observe` was caused by
DISPLAY, XTEST, Screen Recording, Accessibility, window enumeration, or capture
without writing a script or reading source.

## Scope

- Add `niucode computer doctor [--json]`.
- Linux: connect, report XTEST, enumerate windows, capture the first window,
  and show image dimensions/bytes.
- macOS: report Screen Recording and Accessibility, enumerate windows when
  permitted, and capture the first window.
- Actionable errors for missing DISPLAY/TCC; no click/type/key probe.
- Xvfb test invokes the real binary and asserts platform/XTEST/image JSON.

## Acceptance

- [x] Missing DISPLAY returns an actionable error.
- [x] Linux reports XTEST and at least one window/capture under Xvfb.
- [x] macOS reports both TCC permissions before capture.
- [x] JSON and human output exist.
- [x] The doctor is read-only (no input injection).
- [x] Tests/clippy pass.

## Notes

The doctor does not grant TCC permissions; it only reports them. A capture probe
is cheap and ephemeral; it never writes an attachment.
