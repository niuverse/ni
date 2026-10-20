# ni

Minimal, observable coding agent.

`ni` is a small agent harness built around an append-only event log, a thin
provider layer, and a sharp tool surface. It is intentionally smaller than a
general agent framework: one loop, six tools, and a session file you can read,
replay, and resume.

## Status

M0 · Provider. The workspace, session log, CLI, offline dashboard, and
OpenAI-compatible provider (with a DeepSeek preset) exist. The tools and turn
loop are next.

## Quick start

Requires Rust 1.80+.

```sh
cargo run -p ni                 # banner and current status
cargo run -p ni -- session new  # create a session, print its path
cargo run -p ni -- session show <path>
cargo run -p ni -- provider list
DEEPSEEK_API_KEY=... cargo run -p ni -- provider test --preset deepseek

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
  ni-provider/ OpenAI-compatible SSE, DeepSeek preset, scripted provider
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
5. Be honest about sandbox boundaries. In-process path checks are not isolation.

More detail: [architecture](docs/architecture.md) · [provider](docs/provider.md) · [roadmap](docs/roadmap.md) · [ADR-0001: Rust](docs/adr/0001-rust-core.md) · [ADR-0002: event log](docs/adr/0002-event-log.md).

## Dashboard

`docs/index.html` is generated from `docs/dashboard/project.json`, `docs/board/*.md`,
and the Markdown documents. It is read-only in the browser; status changes go
through the repository. See [the maintenance guide](docs/README.md).
