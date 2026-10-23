+++
id = "publish-0.1.8-dry-run"
title = "0.1.8 publish dry run"
kind = "evidence"
status = "living"
order = 72
summary = "Pre-publication checks for 0.1.8: version/license/token/order, the new protocol package, and the expected dependency-order blocker."
+++

## `publish-niucode.sh --check`

```text
version: 0.1.8
license: license = "MIT OR Apache-2.0"
token:   /home/galbot/.cargo/credentials.toml
order:   niucode-core niucode-computer niucode-computer-x11
         niucode-computer-macos niucode-attachments niucode-auth
         niucode-provider niucode-tools niucode-extension niucode-mcp
         niucode-agent niucode
check only; pass --execute to publish
```

## Package check

`cargo package -p niucode-computer --allow-dirty --no-verify` succeeded:

```text
Packaged 6 files, 35.5KiB (9.0KiB compressed)
```

`niucode-computer-x11` cannot package before its dependency exists in the
registry:

```text
no matching package named `niucode-computer` found
```

This is the expected publish-order blocker, not a source failure. Publish
`niucode-computer` first, wait for the sparse index, then package/publish X11
and macOS.

## Not done

No `cargo publish` ran. Publication needs an explicit maintainer go-ahead.
