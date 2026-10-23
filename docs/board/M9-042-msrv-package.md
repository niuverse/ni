+++
id = "M9-042"
title = "MSRV-clean and package-verifiable web stack"
status = "done"
milestone = "M9"
module = "cli"
size = "S"
priority = "P0"
updated = "2026-10-23"
completed = "2026-10-23"
depends_on = ["M9-041"]
docs = ["release"]
+++

## Problem

`cargo +1.88.0 check --workspace --all-targets` emitted five unused-assignment
warnings in the TUI. More seriously, `cargo package -p niucode` failed because
the web approval path serialized `ChangePreview`, a derive added in the local
niucode-tools but absent in the registry version used by the package.

## Scope

- Remove the redundant `line_open`/`thinking` assignments in `tui.rs`.
- Replace direct preview serialization with a local `preview_json` projection
  over the public preview types.
- Verify the package tarball includes `assets/web.html` and `src/web.rs`.
- Re-run MSRV and package checks.

## Acceptance

- [x] `cargo +1.88.0 check --workspace --all-targets` has no warnings.
- [x] `cargo package -p niucode --allow-dirty` verifies.
- [x] Web approval JSON keeps the same diff/command/note shape.
- [x] Web tests still pass.
- [x] No behavior change to terminal output.

## Notes

The package verifies at the current unreleased version. A real release still
needs coordinated crate version bumps and publication order (tools/core before
the CLI).
