+++
id = "release-runbook"
title = "Release runbook: 0.1.8 / niuse 0.1.14"
kind = "guide"
status = "living"
order = 74
summary = "Exact publish order, index waits, clean-install checks, rollback, and the human approval gate for the computer/multimodal release."
+++

## Preconditions

- Both repositories are clean and CI is green.
- `niucode.rev` in niuse matches the final niucode commit.
- `cargo test --workspace`, clippy, dashboard, and product checks pass.
- `./scripts/release-preflight.sh --skip-package` passes before publication.
- A maintainer explicitly authorizes the irreversible publish.
- The crates.io token is present; never print it.
- The three new names are unclaimed (checked 2026-10-24: HTTP 404).

## 1 · Publish niucode 0.1.8

From `niucode/`:

```sh
./scripts/publish-niucode.sh --check
./scripts/publish-niucode.sh --execute
```

The script publishes in dependency order and skips versions already present:

```text
niucode-core
niucode-computer
niucode-computer-x11
niucode-computer-macos
niucode-attachments
niucode-auth
niucode-provider
niucode-tools
niucode-extension
niucode-mcp
niucode-agent
niucode
```

It waits between crates for the sparse index. Do not rerun in parallel.

## 2 · Verify the kernel

```sh
cargo install niucode --version 0.1.8 --locked
niucode --version
niucode exec --replay <fixture> "smoke"   # no API key
cargo package -p niucode --allow-dirty   # now resolves 0.1.8 deps
```

Also verify the registry index shows every crate at 0.1.8, especially the new
`niucode-computer*` crates.

## 3 · Publish niuse 0.1.14

Only after niucode 0.1.8 resolves from the registry:

```sh
cd niuse
./scripts/sync-niucode.sh <final-niucode-commit>
cargo test --workspace
./scripts/release-preflight.sh
./scripts/publish-niuse.sh --check
./scripts/publish-niuse.sh --execute
```

`niuse` must depend on `niucode 0.1.8`; the full preflight includes package
verification.

## 4 · Verify the product

```sh
cargo install niuse --version 0.1.14 --locked
niuse doctor
niuse --computer observe   # only on a real desktop; doctor first
```

Install the opt-in features only if wanted:

```sh
cargo install niuse --version 0.1.14 --features web,tui --locked
```

## 5 · Tag and evidence

- Create tags only after clean installs succeed.
- Record registry versions, install output, and `niucode --version`.
- Update `docs/evidence/` and the product release notes with actual results.
- Do not claim real-desktop/macOS acceptance without the manual checklist.

## Rollback

- **Do not delete or rewrite published versions.**
- `cargo yank --vers 0.1.8 -p <crate>` stops new resolution but does not remove
  downloads; yank the whole set coherently.
- Publish a fixed patch version rather than reusing 0.1.8.
- If niuse is broken but niucode is fine, yank niuse 0.1.14 and publish 0.1.15.
- Record the incident and the yanked versions in the release notes.

## Explicitly not automated

No script publishes without `--execute`; the release workflow is tag-triggered
and still requires repository secrets. A human must approve.
