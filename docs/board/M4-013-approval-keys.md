+++
id = "M4-013"
title = "Configurable approval keys for embedders"
status = "done"
milestone = "M4"
module = "tui"
size = "S"
priority = "P2"
updated = "2026-10-22"
completed = "2026-10-22"
depends_on = ["M4-001"]
docs = ["tui"]
+++

## Problem

The approval pager and the inline approval view hardcode `a` allow, `d` deny,
`s` session. An embedder with its own documented keys cannot align its front
end. niuse's REPL uses `y` allow, `n` deny, and `d` for "show the full diff";
in the TUI, `d` denies. A user who learned one UI can deny a write by accident in
the other.

## Scope

- `ApprovalKeys { allow, deny, session }` with the existing `a`/`d`/`s` default.
- `TuiOptions.approval_keys`; duplicate keys are rejected.
- The pager, the inline approval view, the footer, and the line fallback render
  and honor the configured keys.
- `Esc`, `q`, and Ctrl-C keep their safety meaning: deny or cancel.

## Acceptance

- [x] Custom keys approve, deny, and allow-for-session in the pager.
- [x] The inline view and its footer use the same keys.
- [x] The line fallback accepts the configured first letter.
- [x] Duplicate keys are rejected at construction.
- [x] The default is unchanged and the full suite passes.

## Notes

niuse sets `y`/`n`/`s`, so `d` no longer denies on that path. The full diff is
already visible in the pager, which is why niuse does not need a `d` action.
