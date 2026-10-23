+++
id = "release-0.1.7"
title = "Release 0.1.7 (draft)"
kind = "release"
status = "draft"
order = 70
summary = "Draft notes for the next niucode release: local web surface, streaming Markdown, approvals, TUI rendering, and verification evidence."
+++

## Status

Published 2026-10-23. The workspace is `0.1.7`; all eight publishable crates are
on crates.io, verified by the registry index and a clean `cargo install
niucode --version 0.1.7`.

## Highlights

- **Local web surface** (`niucode web --session <jsonl>`): loopback-only HTTP +
  SSE, embedded client, no build step, no remote assets. See ADR-0036.
- **Browser chat**: read-only/workspace-write turns, streamed Markdown and
  MathML, visible approvals with structured diffs, tool cards, session export,
  transcript search, prompt history, and slash commands.
- **TUI rendering**: inline Markdown, tables with styled cells, lists/tasks,
  GFM alert labels/tones, viewport rules, math fallback, code-fence labels,
  token highlighting, and multi-line code comment/string state.
- **Motion/progress**: streaming caret, per-tool elapsed footer, compact
  per-tool markers, reduced-motion env fallbacks.
- **Web Markdown**: task lists, footnotes, GFM alerts, safe href allowlist,
  remote-image stripping, stream throttling, whole-message/block copy.
- **Web security**: Host/Origin guards, strict CSP, per-run token + HttpOnly
  cookie, 1MiB complete-body reads, 5s request timeout, threat model.
- **Web status/context**: model/cwd/policy chips, turn timer, token usage,
  collapsible context sidebar (mobile overlay), `context_changed` refresh.
- **Approvals**: diff previews, Allow/Deny, Allow tool for the session,
  5:00 countdown.

## Verification

Publish dry run: all eight publishable crates package/verify; nothing uploaded
([evidence](evidence/publish-dry-run.md)).

```sh
cargo +1.88.0 check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo package -p niucode --allow-dirty
```

Last run: MSRV clean, workspace tests/clippy green, package verifies and
includes `assets/web.html` and `src/web.rs`.

## Release order

1. Bump all niucode workspace crates to `0.1.7`; update sibling version
   requirements.
2. Publish foundation crates first (`niucode-core`, `niucode-auth`,
   `niucode-provider`, `niucode-tools`, `niucode-agent`, ...), then
   `niucode-mcp`, `niucode`, if the release includes those changes.
3. Verify `cargo install niucode --version 0.1.7`.
4. Bump niuse to `0.1.13`, require `niucode = 0.1.7`, publish.
5. Update `niuse/niucode.rev` and `docs/compat.md`.

## Blockers

- None. Published with explicit human approval.
- `niuse 0.1.13` now requires this release; its `web` feature verifies against
  the registry.
