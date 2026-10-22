# niucode

Minimal, observable coding agent.

`niucode` is a small agent harness built around an append-only event log, a thin
provider layer, and a sharp tool surface. It is intentionally smaller than a
general agent framework: one loop, six tools plus a read-only subagent, and a session file you can read,
replay, and resume.

## Status

M0–M3 complete; M4 in progress. M2 closed with layered `AGENTS.md` discovery, a
deterministic 75%-budget compaction handoff, instruction digests, and per-turn
cache hit rates. `niucode exec` and `niucode tui` share six tools plus a read-only subagent, bwrap
isolation, per-tool approval with full unified-diff review, cancellation, resume,
and recorded fixtures. The TUI has a Geek-style wordmark, symbol-based tool
lines, slash commands (`/init`, `/clear`, `/plan`, `/goal`, `/fork`, `/status`,
`/session`), an automatic command menu with mouse hover and descriptions,
reverse history search, plan/goal prompt badges, and a resize-aware diff pager;
long tool output folds behind `:last`.
Replay and fork are available offline with stable `e000012` event IDs; read-only
subagents run in parallel with independent session logs; `exec --json` exposes a
versioned JSONL protocol. M4 is active: profiles, `/plan`, `/goal`, and the
sidecar extension host are landing incrementally; M4-002 loads local
`.niucode/extensions/*.toml` tools through JSON-RPC.

## Embedding

`niucode-cli` builds a library as well as the binary. `run()` starts the normal CLI, `run_tui_with_hooks(TuiOptions, Box<dyn TurnHooks>)` runs the TUI with prompt, event, and turn-end hooks, and `run_exec_with_hooks(ExecOptions, &mut dyn TurnHooks)` runs a one-shot task with the same three hooks. `examples/hooked_tui.rs` is a minimal recording probe; it also exercises `filter_text` before display. See [ADR-0032](docs/adr/0032-embedding-library-surface.md).

## Quick start

Requires Rust 1.88+. On Linux, install `bubblewrap` for sandboxed bash; without it, the default `--approval on-failure` falls back to per-tool prompts. The OS keyring uses Secret Service (`libdbus-1-dev` at build time); use `NIUCODE_AUTH_DISABLE_KEYRING=1` for the 0600 file fallback.

```sh
cargo install niucode                # published; Rust 1.88+
cargo install niucode --version 0.1.2 --locked
cargo run -p niucode                 # start the TUI from a checkout (no subcommand needed)
cargo run -p niucode -- session new  # create a session, print its path
cargo run -p niucode -- session show <path>
cargo run -p niucode -- auth set deepseek
cargo run -p niucode -- auth status
cargo run -p niucode -- provider list
cargo run -p niucode -- provider test --preset deepseek

cargo run -p niucode -- exec "read a.txt and summarize it" --no-thinking
cargo run -p niucode -- exec "continue" --resume /path/to/session.jsonl
cargo run -p niucode -- exec "replay" --replay /path/to/fixture.json
cargo run -p niucode -- tui
cargo run -p niucode -- exec "review only" --sandbox read-only
cargo run -p niucode -- exec "fix it" --sandbox workspace-write --network off --approval ask
cargo run -p niucode -- exec "trusted" --sandbox full-access --network on --approval never
cargo run -p niucode -- exec "long task" --context-budget 128000 --keep-recent-turns 4
cargo run -p niucode -- exec "read a.txt" --json            # JSONL event protocol

cargo test --workspace
cargo xtask dashboard           # regenerate docs/index.html
cargo xtask dashboard --check   # fail if the generated dashboard is stale
```

The dashboard is a single offline HTML file. Architecture is the first screen;
Work, Docs, and Decisions are secondary views. Open `docs/index.html` directly;
no server, CDN, or network access is required.

## Layout

```text
crates/
  niucode-core/     event model and append-only JSONL session log
  niucode-auth/     env → OS keyring → 0600 file credential resolution
  niucode-provider/ OpenAI-compatible SSE, DeepSeek preset, scripted provider
  niucode-tools/    six tools, path policy, bwrap sandbox, approval policy
  niucode-agent/    turn loop, tool dispatch, context, subagents, session events
  niucode-cli/      argument parsing, sessions, provider checks
  xtask/      dashboard builder and repository checks
docs/
  index.html  generated project dashboard
  board/      one markdown file per task card
  adr/        architecture decision records
  dashboard/  dashboard source: project model, template, CSS, JS
```

## Principles

1. The event log is the only source of truth. In-memory state is a fold of it.
2. Model context is a lossy projection of the log, not a second transcript.
3. Keep the tool surface small; every tool is prompt surface and failure surface.
4. Keep the provider layer thin; normalize only what the loop needs.
5. Be honest about sandbox boundaries. File tools use in-process path checks;
   bash uses bwrap on Linux, and missing bwrap downgrades to explicit approval.

More detail: [architecture](docs/architecture.md) · [exec](docs/exec.md) · [provider](docs/provider.md) · [auth](docs/auth.md) · [roadmap](docs/roadmap.md) · [ADR-0001: Rust](docs/adr/0001-rust-core.md) · [ADR-0002: event log](docs/adr/0002-event-log.md) · [ADR-0006: sandbox](docs/adr/0006-sandbox.md).

## Dashboard

`docs/index.html` is generated from `docs/dashboard/project.json`, `docs/board/*.md`,
and the Markdown documents. It is read-only in the browser; status changes go
through the repository. See [the maintenance guide](docs/README.md).

## License

MIT OR Apache-2.0. See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE).
