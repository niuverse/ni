+++
id = "M9-115"
title = "Sensitive-window observation requires approval"
status = "done"
milestone = "M9"
module = "loop"
size = "S"
priority = "P0"
updated = "2026-10-24"
completed = "2026-10-24"
depends_on = ["M9-099"]
docs = ["adr-0037"]
+++

## Problem

`observe` was allowed in `observe`/`control`/`autonomous` without approval.
A screenshot of a password manager or authentication window could therefore
silently reach the model and the provider.

## Scope

- After capture, if the backend classified the window as `Sensitive`, request
  approval before returning the image.
- On denial, discard the image, return a failed ToolResult, and append
  `Event::Approval { allowed:false }`.
- Sanitize/truncate app/title/purpose in the approval summary.
- Keep terminal windows observable (their input remains approval-gated).
- Add a regression for denial and no image part.

## Acceptance

- [x] Sensitive observe asks once in `observe` policy.
- [x] Denial sends no image to the provider.
- [x] Denial is audited.
- [x] The summary strips control characters and bounds text.
- [x] Non-sensitive observe is unchanged.
- [x] Tests/clippy pass.

## Notes

This is an additional privacy gate, not a policy tier change. The model still
must call observe; there is no background capture.
