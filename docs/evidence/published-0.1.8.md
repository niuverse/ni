+++
id = "published-018"
title = "Published 0.1.8 evidence"
kind = "evidence"
status = "living"
order = 76
summary = "All twelve niucode 0.1.8 crates are on crates.io; clean install reports 0.1.8."
+++

## Registry

Checked 2026-10-24 after the publish script completed:

```text
niucode-core 0.1.8
niucode-computer 0.1.8
niucode-computer-x11 0.1.8
niucode-computer-macos 0.1.8
niucode-attachments 0.1.8
niucode-auth 0.1.8
niucode-provider 0.1.8
niucode-tools 0.1.8
niucode-extension 0.1.8
niucode-mcp 0.1.8
niucode-agent 0.1.8
niucode 0.1.8
```

## Clean install

```sh
cargo install niucode --version 0.1.8 --locked --root /tmp/niucode-install-018
/tmp/niucode-install-018/bin/niucode --version
```

Result:

```text
niucode 0.1.8
```

## Package verification

After the registry contained all dependencies:

```sh
cargo package -p niucode --allow-dirty
```

Packaged 47 files, 772.3KiB; verification compiled and finished successfully.

## Publish command

`scripts/publish-niucode.sh --execute` completed with exit 0. It published in
the documented order, skipping already-present versions.
