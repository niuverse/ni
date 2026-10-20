# ni

Minimal, observable coding agent.

`ni` is a small agent harness built around an append-only event log, a thin
provider layer, and a sharp tool surface. It is intentionally smaller than a
general agent framework: one loop, six tools, and a session file you can read,
replay, and resume.

## Status

M1 · sandbox landed. `ni exec` has six tools, bwrap workspace/network isolation,
per-tool approval, process-group cancellation, resume, recorded fixtures, and
auth. The next card is the in-process TUI.

## Quick start

Requires Rust 1.85+. On Linux, install `bubblewrap` for sandboxed bash; without it, the default `--approval on-failure` falls back to per-tool prompts. The OS keyring uses Secret Service (`libdbus-1-dev` at build time); use `NI_AUTH_DISABLE_KEYRING=1` for the 0600 file fallback.

```sh
cargo run -p ni                 # banner and current status
cargo run -p ni -- session new  # create a session, print its path
cargo run -p ni -- session show <path>
cargo run -p ni -- auth set deepseek
cargo run -p ni -- auth status
cargo run -p ni -- provider list
cargo run -p ni -- provider test --preset deepseek

cargo run -p ni -- exec "read a.txt and summarize it" --no-thinking
cargo run -p ni -- exec "continue" --resume /path/to/session.jsonl
cargo run -p ni -- exec "replay" --replay /path/to/fixture.json
cargo run -p ni -- exec "review only" --sandbox read-only
cargo run -p ni -- exec "fix it" --sandbox workspace-write --network off --approval ask
cargo run -p ni -- exec "trusted" --sandbox full-access --network on --approval never

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
  ni-core/    event model and append-only JSONL session log
  ni-auth/    env → OS keyring → 0600 file credential resolution
  ni-provider/ OpenAI-compatible SSE, DeepSeek preset, scripted provider
  ni-tools/   six tools, path policy, bwrap sandbox, approval policy
  ni-agent/   turn loop, tool dispatch, session events
  ni-cli/     argument parsing, sessions, provider checks
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
