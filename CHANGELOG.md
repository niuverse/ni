# Changelog

## 0.1.0 — 2026-10-22

First public crate set, versioned together at 0.1.0.

### Kernel
- `niucode-core`: event model, append-only session logs, compaction, forward-compatible reads.
- `niucode-provider`: OpenAI-compatible streaming providers.
- `niucode-tools`: file, shell, and patch tools with sandbox policy.
- `niucode-extension`: extension and hook surface.
- `niucode-auth`: credential storage.
- `niucode-agent`: turn loop, history projection, approvals, cancellation, and `TurnHooks` sketch.

### Reference UI
- `niucode` binary with the ratatui TUI, CLI one-shot mode, approval flow, and session resume.
- Session files are created `0600`; unknown events warn while readable lines survive.
- Linux sandbox and macOS checks in CI; MSRV 1.88.
