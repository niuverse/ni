+++
id = "M9-118"
title = "Bound typed text and key names"
status = "done"
milestone = "M9"
module = "tools"
size = "S"
priority = "P1"
updated = "2026-10-24"
completed = "2026-10-24"
depends_on = ["M9-086"]
docs = ["adr-0037"]
+++

## Problem

The schema advertised no text length limit, and the runtime accepted arbitrarily
large `type` text and key names. A hostile or confused model could make XTEST
loop over megabytes of characters.

## Scope

- Add `MAX_TYPE_CHARS = 10_000` and `MAX_KEY_CHARS = 64`.
- Reject oversized actions during observation validation, before backend
  dispatch.
- Publish the limits in the tool schema (`maxLength`).
- Keep scroll's existing 1–50 bound.
- Unit tests for both errors.

## Acceptance

- [x] Oversized typed text is rejected.
- [x] Oversized key names are rejected.
- [x] Schema advertises the limits.
- [x] Rejection happens before XTEST/CGEvent.
- [x] Tests/clippy pass.

## Notes

The limit is per action, not per turn; a model can still send many bounded
actions subject to policy/approval and step limits.
