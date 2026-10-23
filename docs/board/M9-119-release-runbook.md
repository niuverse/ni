+++
id = "M9-119"
title = "Release runbook and name availability"
status = "done"
milestone = "M9"
module = "cli"
size = "S"
priority = "P1"
updated = "2026-10-24"
completed = "2026-10-24"
depends_on = ["M9-113"]
docs = ["release-runbook", "release-0.1.8"]
+++

## Problem

The release steps existed in notes and scripts but not as one ordered runbook
covering publish order, index waits, clean installs, rollback, and the human
gate.

## Scope

- Add `docs/release-runbook.md` for 0.1.8 / 0.1.14.
- Record the new crate names as unclaimed via the crates.io API.
- Document yank/rollback (never delete/rewrite versions).
- Link the runbook from the release card.
- No publication is performed.

## Acceptance

- [x] Publish order matches `publish-niucode.sh`.
- [x] Verification and clean-install steps are explicit.
- [x] Rollback is yank + patch, not rewrite.
- [x] Human approval is a precondition.
- [x] New names returned 404 before publication.
- [x] Docs/dashboard pass.

## Notes

The runbook is intentionally explicit about the irreversible step; it does not
authorize itself.
