+++
id = "release"
title = "Releasing niucode crates"
kind = "doc"
status = "living"
order = 55
summary = "Version 0.1.0, publish order, and the license/token prerequisites for the first public crates."
+++

## Version

All publishable crates share the workspace version **0.1.0**. Treat them as one release: bump `[workspace.package] version` and publish the whole set together. niuse will depend on `niucode-* = "0.1.0"` while keeping path dependencies for local development.

Version `0.1.0` signals the same thing as niuse's 0.1.0: usable, documented, still pre-1.0.

## Prerequisites

1. **A license.** The workspace currently has none. crates.io expects license metadata; choose one (MIT OR Apache-2.0 is the usual Rust default), add it to `[workspace.package]`, and commit it before publishing.
2. **A crates.io token.** `cargo login`, or set `CARGO_REGISTRY_TOKEN` in the environment. Never commit the token.
3. **A clean tree and green CI.** Publish from a commit that passed the normal workflow; no `--allow-dirty`.

## Publish order

Each crate must exist in the registry before its dependents can resolve it:

```text
niucode-core
niucode-auth
niucode-provider
niucode-tools
niucode-extension
niucode-agent
niucode          (binary crate)
```

`xtask` and `benchmarks/gateway` are `publish = false`.

## Commands

```sh
./scripts/publish-niucode.sh --check     # license/token/plan, no upload
./scripts/publish-niucode.sh --execute   # publish in order, waiting for the index
```

A dry run of a dependent crate cannot succeed before its dependency is published: Cargo looks up `niucode-core` on crates.io. That is why the script checks the plan instead of pretending each crate can be dry-run locally.

## After publishing

- Add `version = "0.1.0"` beside the `path` dependencies in niuse.
- Bump niuse's `niucode.rev` to the release commit.
- Verify `cargo install niucode --version 0.1.0` in a clean directory.
- Then niuse can be packaged and published on the same version.

## If a release is wrong

Yank the affected version (`cargo yank --version 0.1.0 <crate>`) and publish `0.1.1`; do not reuse a version.
