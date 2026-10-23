+++
id = "M9-079"
title = "Publish dry run for all crates"
status = "done"
milestone = "M9"
module = "cli"
size = "S"
priority = "P0"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-052"]
docs = ["release"]
+++

## Problem

Release packaging was only checked for the `niucode` CLI. Foundation crates and
the embedded asset could still fail after a version bump.

## Scope

- `cargo publish --dry-run` for all eight publishable crates in dependency order.
- Record package/verify/upload status in evidence.
- Do not upload, bump versions, or tag.
- State what the dry run does and does not prove.

## Acceptance

- [x] All eight crates package and verify.
- [x] Upload is aborted by dry-run for each.
- [x] Evidence lists the exact command and HEAD.
- [x] Embedded assets are covered by the CLI package.
- [x] Human approval remains the release gate.

## Notes

The dry run uses the current `0.1.6` versions; a release still needs a
coordinated bump and registry publication.
