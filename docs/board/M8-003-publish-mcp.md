+++
id = "M8-003"
title = "Publish niucode-mcp at 0.1.5"
status = "doing"
milestone = "M8"
module = "mcp"
size = "S"
priority = "P1"
updated = "2026-10-23"
depends_on = ["M8-001"]
docs = ["release", "mcp"]
+++

## Problem

niuse cannot package its current tree because `niucode-mcp` is not on crates.io.
The crate is release-ready, but the upload is a human action.

## Scope

- `cargo package -p niucode-mcp` and `cargo publish -p niucode-mcp --dry-run`.
- Add it to the publish order between extension and agent.
- Keep the version at 0.1.5; it is a new crate, so existing versions do not move.
- After upload, verify `cargo package -p niuse` against the registry.

## Acceptance

- [x] `cargo package -p niucode-mcp` passes.
- [x] `cargo publish -p niucode-mcp --dry-run` reaches the dry-run upload step.
- [x] The publish script and release doc include the crate in order.
- [x] The crate is published on crates.io (2026-10-23).
- [ ] `cargo package -p niuse` passes against the registry. It currently fails because crates.io `niuse-core` 0.1.11 predates the plan API; niuse 0.1.12 is the next release.

## Notes

The repository has a crates.io token configured, but this card does not
authorize an upload. Publishing needs an explicit human decision.
