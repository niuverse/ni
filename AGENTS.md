# AGENTS.md

This repository is `niucode`, a minimal coding-agent harness. Keep it small, legible,
and observable. The project dashboard in `docs/index.html` is part of the
product, not decoration.

## Read first

1. `docs/dashboard/project.json` — milestones, modules, relations, timeline.
2. `docs/board/` — current cards and acceptance criteria.
3. `docs/architecture.md` — boundaries and non-goals.
4. `docs/adr/` — accepted decisions. Do not silently reverse one.

## Commands

```sh
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo xtask dashboard
cargo xtask dashboard --check
```

`cargo xtask` is a workspace alias defined in `.cargo/config.toml`.

## Hard rules

- **The event log is the only durable truth.** Never invent a second session
  state that survives restarts. `niucode-core` owns `Event` and `SessionLog`.
- **Context is a projection.** Compaction, truncation, and prompt assembly read
  the log; they never rewrite it.
- **Dependency direction:** `niucode-core` does not depend on CLI, TUI, providers, or
  tools. UI consumes events; it does not call loop internals.
- **Tool surface stays small.** Add a tool only when an existing one cannot do
  the job. Every tool description is prompt surface.
- **Writes are serialized.** Read-only tools may run concurrently; mutating tools
  must not race on the same file.
- **Cancellation is mandatory.** Anything that can block accepts an abort signal;
  shell commands kill their process group.
- **Do not hand-edit** `docs/index.html` or `docs/dashboard/model.json`. Edit the
  sources under `docs/` and rebuild.
- **Credentials go through `niucode-auth`.** Never read API keys directly in new
  call sites, print them, or append them to a session event. Resolution order is
  env → OS keyring → 0600 file.
- **Read before write.** This applies to the agent and to the filesystem policy:
  do not patch a file that has not been observed in the current session.

## Definition of done

A task is not done until:

- [ ] The behavior is implemented, not stubbed.
- [ ] `cargo test --workspace` passes; new behavior has a focused test.
- [ ] `cargo fmt --all --check` and clippy pass.
- [ ] The matching card in `docs/board/` has the right `status`, `updated`, and
      `completed` (for done).
- [ ] A decision that changes architecture has an ADR and a `decision` event.
- [ ] Module boundaries or owned paths changed → `project.json` updated.
- [ ] `cargo xtask dashboard` was run and the regenerated `docs/index.html` is included.
- [ ] The change does not widen the core without an explicit reason.

## Dashboard conventions

- Architecture is the first screen. Navigation is Architecture / Work / Docs /
  Decisions. Do not reintroduce progress bars, completion percentages, or card
  counts into Architecture.
- Work is a secondary handoff surface: cards and statuses, no progress chrome.
- Decisions contains ADRs and `kind = "decision"` events only.
- One card per file in `docs/board/`, TOML frontmatter + Markdown body.
- Card IDs are stable: `M0-001`, `M1-002`, … Never renumber.
- `status`: `backlog | ready | doing | review | done | blocked`.
- `done` requires `completed`; `blocked` requires `blocked_reason`.
- `project.json` modules are authored, not inferred from folders. `active` and
  `done` modules must own at least one real path.
- Keep the browser dashboard read-only, framework-free, and a single offline
  file. No React/Svelte/Vite, zoom/pan, or drag-editing without an ADR.
- A drag-and-drop writer would create a second source of truth and is out of
  scope.

## Current focus

M0–M4 are done; M5 is active. M4-001/M4-002/M4-003 are done: CommandRegistry,
TUI slash auto-menu, `/plan`, `/goal`, the session-scoped sidecar extension
host, and subagent Explore/Worker profiles with W1 write serialization.
M4-004/M4-005/M4-006/M4-007/M4-008/M4-009/M4-010/M4-011/M4-012 are done.
M5-001/M5-002/M5-004/M5-005 are done; M5-003 inline is review. Ghostty
currently defaults to the stable linear editor; linear startup overlap was
fixed by unifying startup output/flush; inline remains M5-006 backlog. There is no project-local
`.niucode`; state lives in `~/.niucode/projects/<key>/`. skills discovery/trust is done (M5-007); tool metadata/LoopPhase and the
interactive `/resume` picker are done.
`/goal run` is available in TUI and headless via `exec --goal`. TUI mouse capture
is menu-scoped, terminal state is restored by `TerminalGuard`, session
lifecycle is `/clear` vs `/reset` vs `/resume`, and runtime selection is
`/model` / `/permissions` (ADR-0015), with dropdown argument completion. Keep the event log
as the only durable state; plugins are capability-scoped and out-of-process,
never a second durable store.
