+++
id = "publish-dry-run"
title = "Publish dry run · niucode 0.1.6"
kind = "doc"
status = "done"
order = 43
summary = "All eight publishable niucode crates package and verify under cargo publish --dry-run; nothing was uploaded."
+++

## Context

Date: 2026-10-23. HEAD: `3adc37f`. Workspace versions: `0.1.6`.
This is packaging verification for the next coordinated release, not a publish.

## Command

```sh
for crate in niucode-core niucode-auth niucode-provider niucode-tools \
             niucode-agent niucode-extension niucode-mcp niucode; do
  cargo publish --dry-run -p "$crate" --allow-dirty
done
```

## Result

| Crate | Package | Verify | Upload |
| --- | --- | --- | --- |
| niucode-core | pass | pass | aborted (dry run) |
| niucode-auth | pass | pass | aborted (dry run) |
| niucode-provider | pass | pass | aborted (dry run) |
| niucode-tools | pass | pass | aborted (dry run) |
| niucode-agent | pass | pass | aborted (dry run) |
| niucode-extension | pass | pass | aborted (dry run) |
| niucode-mcp | pass | pass | aborted (dry run) |
| niucode | pass | pass | aborted (dry run) |

Every crate reported `warning: aborting upload due to dry run`.

## What this proves

- Each crate packages the expected files.
- Each packaged crate compiles in isolation enough for Cargo's verification.
- The embedded `assets/web.html` is present in the `niucode` package.
- The publish order is mechanically plausible.

## What this does not prove

- It does not upload, tag, or bump versions.
- It does not prove crates.io index propagation or `cargo install`.
- It does not replace human approval for the release.
- `xtask` and `niucode-bench-gateway` are not published.
