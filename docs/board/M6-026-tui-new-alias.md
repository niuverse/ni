+++
id = "M6-026"
title = "TUI /new alias for /clear"
status = "done"
milestone = "M6"
module = "tui"
size = "S"
priority = "P1"
updated = "2026-10-22"
completed = "2026-10-22"
depends_on = ["M6-001"]
docs = ["tui"]
+++

## Problem

niuse's REPL has `/new`, but the TUI answered `unknown command /new`; only
`/clear` existed. A PTY regression for `/new` found the mismatch, and a real TUI
session had silently continued in the old thread because the command was not
recognized.

## Scope

- Parse `/new` as `Command::Clear`; `/clear` remains canonical.
- Update the help description to name the alias.
- Reject extra arguments with the same `/clear` usage error.
- Parser and PTY regressions.

## Acceptance

- [x] `/new` maps to `Command::Clear`.
- [x] `/new extra` is a usage error for `/clear`.
- [x] The help line mentions the alias.
- [x] A PTY test starts a new thread with `/new`, archives the old one, and
      proves the old answer does not leak.

## Notes

The alias is display-free in completion (only `/clear` is listed), but accepted
when typed. This keeps the canonical command stable while matching the REPL.
