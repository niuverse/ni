+++
id = "release-0.1.7"
title = "Release 0.1.7 (draft)"
kind = "release"
status = "draft"
order = 70
summary = "Draft notes for the next niucode release: local web surface, streaming Markdown, approvals, TUI rendering, and verification evidence."
+++

## Status

Draft. Not published. Version numbers in the workspace are still 0.1.6; a
release requires a coordinated bump and publication order.

## Highlights

- **Local web surface** (`niucode web --session <jsonl>`): loopback-only HTTP +
  SSE, embedded client, no build step, no remote assets. See ADR-0036.
- **Browser chat**: read-only/workspace-write turns, streamed Markdown and
  MathML, visible approvals with structured diffs, tool cards, session export,
  transcript search, prompt history, and slash commands.
- **TUI rendering**: inline Markdown, tables with styled cells, lists/rules,
  math fallback, code-fence labels, token highlighting, and multi-line code
  comment/string state.
- **Motion/progress**: streaming caret, per-tool elapsed footer, compact
  per-tool markers.
- **Web status/context**: model/cwd/policy chips, turn timer, token usage,
  product context sidebar, `context_changed` refresh.

## Verification

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

- Human approval to publish.
- Version bump has not been applied.
- Registry package verification cannot pass for a new sibling API until the
  foundation crates are published; the CLI package itself verifies at the
  current version.
