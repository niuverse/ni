+++
id = "release-0.1.8"
title = "Release 0.1.8 (draft)"
kind = "release"
status = "draft"
order = 71
summary = "Draft notes: native Linux X11 and macOS computer use, observation-bound input, approvals/emergency stop, and multimodal attachment memory."
+++

## Status

Prepared 2026-10-23. The workspace is `0.1.8`; publication is pending an
explicit maintainer go-ahead. Nothing has been uploaded for this version.

## Highlights

- **Native computer use**: new `niucode-computer` protocol, Linux X11
  (`x11rb` + XTEST), and macOS (CoreGraphics + CGEvent) backends.
- **Observation-bound actions**: click/type/key/scroll carry an observation
  id, window id, image-relative coordinates, expiry, and id/app/class/kind +
  geometry rechecks before dispatch.
- **Permissions**: `off | observe | control | autonomous`, explicit arm,
  per-action approval for control, terminal/sensitive red lines, Ctrl-C
  emergency stop and input release.
- **Agent tool**: `computer` returns the screenshot as a tool image part;
  action failures stay ToolResults; approvals show safe action details and are
  audited, typed text is persisted only as length + SHA-256 unless
  `--computer-log-input` is explicit, and screenshots stay ephemeral unless
  `--computer-keep-frames` is explicit.
- **Product surfaces**: `niucode`/`niuse` exec, TUI, REPL, and web expose
  `--computer`; TUI/REPL `/computer stop|arm`, web status chip and
  token-gated arm/stop control.
- **Multimodal attachments**: OpenAI-compatible image parts, capability gate,
  content-addressed store, budgeted text/image turns, a four-image
  per-request visual cap, TUI `/attach`/`@path`, Web upload/drop/paste, CLI
  `--attach`/`--image`.
- **Memory evidence**: context projection, reopened attachment-store history,
  and reopened Markdown memory in a second session.

## Verification

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo xtask dashboard --check
./scripts/publish-niucode.sh --check
```

The Linux X11 path is exercised under Xvfb in CI, including an end-to-end
agent tool turn (observe image → approved click → ButtonPress) and
`niucode computer doctor --json`. Real-desktop and macOS steps live in
[the manual acceptance guide](computer-acceptance.md). macOS compiles and runs unit
tests on the macOS runner; real TCC/dogfood interaction remains manual.

## Release order

1. Publish `niucode-core`, `niucode-computer`, `niucode-computer-x11`,
   `niucode-computer-macos`, `niucode-attachments`, `niucode-auth`,
   `niucode-provider`, `niucode-tools`, `niucode-extension`, `niucode-mcp`,
   `niucode-agent`, then `niucode`.
2. Wait for each new crate to appear in the sparse index before publishing
   dependents.
3. Verify a clean `cargo install niucode --version 0.1.8`.
4. Bump niuse to `0.1.14`, require `niucode = 0.1.8`, publish.

## Caveats

- Pre-release package verification for the new crates cannot succeed against
  registry `0.1.8` until the dependency order has been published.
- macOS runtime evidence requires a signed/permission-granted Mac.
