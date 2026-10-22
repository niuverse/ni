# Changelog

## 0.1.0 — 2026-10-22

First public crate set, versioned together at 0.1.0.

### Kernel
- `niucode-core`: event model, append-only session logs, compaction, forward-compatible reads.
- `niucode-provider`: OpenAI-compatible streaming providers.
- `niucode-tools`: file, shell, and patch tools with sandbox policy.
- `niucode-extension`: extension and hook surface.
- `niucode-auth`: credential storage.
- `niucode-agent`: turn loop, history projection, approvals, and cancellation.

### Reference UI
- `niucode` binary with the ratatui TUI, CLI one-shot mode, approval flow, and session resume.
- Inline TUI clears ratatui's wide-grapheme filler cells, so CJK text no longer renders with a space after every character.
- `niucode-cli` also builds a library with `run()`, `ExecOptions`, `run_exec_with_hooks()`, `TurnHooks`, and `NoHooks`; the exec path invokes prompt, event, and turn-end hooks, while the TUI path is not wired yet.
- Session files are created `0600`; unknown events warn while readable lines survive.
- Linux sandbox and macOS checks in CI; MSRV 1.88.
