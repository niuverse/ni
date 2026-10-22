+++
id = "M8-003"
title = "Publish niucode-mcp at 0.1.5"
status = "ready"
milestone = "M8"
module = "mcp"
size = "S"
priority = "P1"
updated = "2026-10-22"
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
- [ ] The crate is published on crates.io.
- [ ] `cargo package -p niuse` passes against the registry.

## Notes

The repository has a crates.io token configured, but this card does not
authorize an upload. Publishing needs an explicit human decision.
