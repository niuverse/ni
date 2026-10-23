+++
id = "M9-109"
title = "Explicit typed-text logging opt-in"
status = "done"
milestone = "M9"
module = "loop"
size = "S"
priority = "P1"
updated = "2026-10-24"
completed = "2026-10-24"
depends_on = ["M9-107"]
docs = ["adr-0037"]
+++

## Problem

ADR-0037 allowed full typed text only behind an explicit flag, but no such flag
existed; the only behavior was redaction.

## Scope

- Add `AgentConfig.computer_log_input` (default false).
- Add `niucode exec/tui --computer-log-input` and niuse's global flag.
- When enabled, leave `computer type` arguments intact in the assistant event;
  when disabled, keep the existing length + SHA-256 redaction.
- Warn on startup that full desktop text will be persisted.
- Tests for both helper branches and CLI parsing.

## Acceptance

- [x] Default remains redacted.
- [x] Explicit opt-in preserves typed text.
- [x] Redaction metadata still uses chars + SHA-256 when disabled.
- [x] CLI surfaces the flag and warns.
- [x] No web opt-in is added.
- [x] Tests/clippy pass.

## Notes

The opt-in is intentionally noisy and per-run; it should only be used in a
controlled environment. Screenshot persistence remains separate.
