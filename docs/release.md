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

1. **A license.** `MIT OR Apache-2.0` is set in `[workspace.package]`, with `LICENSE-MIT` and `LICENSE-APACHE` at the root. Every publishable crate inherits it.
2. **A verified crates.io email.** crates.io rejects uploads from an account whose email is not verified, even with a valid token. Set it at <https://crates.io/settings/profile>.
3. **A crates.io token.** `cargo login`, or set `CARGO_REGISTRY_TOKEN` in the environment. Never commit the token.
4. **A clean tree and green CI.** Publish from a commit that passed the normal workflow; no `--allow-dirty`.

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

Or run the **release** workflow from Actions → Run workflow, type `publish-niucode`, and let it use the `CARGO_REGISTRY_TOKEN` repository secret. The `crates-io` environment can hold protection rules for that secret. The workflow runs the same script and refuses without a license or token.

The script checks crates.io before each upload and skips a crate whose version is already listed, so a failed run can be retried without `already uploaded` errors. A dry run of a dependent crate cannot succeed before its dependency is published: Cargo looks up `niucode-core` on crates.io. That is why the script checks the plan instead of pretending each crate can be dry-run locally.

## Published

0.1.0 was published on 2026-10-22 in the order above. `cargo install niucode --version 0.1.0 --locked` was verified from a clean root, and the installed binary reports `niucode 0.1.0`. Tag: `v0.1.0`.

## After publishing

- Add `version = "0.1.0"` beside the `path` dependencies in niuse.
- Bump niuse's `niucode.rev` to the release commit.
- Verify `cargo install niucode --version 0.1.0` in a clean directory.
- Then niuse can be packaged and published on the same version.

## If a release is wrong

Yank the affected version (`cargo yank --version 0.1.0 <crate>`) and publish `0.1.1`; do not reuse a version.
