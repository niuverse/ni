+++
id = "M0-001"
title = "Cargo workspace 与工具链"
status = "done"
milestone = "M0"
size = "S"
priority = "P0"
updated = "2026-10-20"
completed = "2026-10-20"
+++

## 范围

- 三个 crate：`ni-core`、`ni-cli`、`xtask`。
- 统一 edition、依赖版本和 workspace lint 入口。
- `cargo xtask dashboard` 作为仓库工具入口。

## 验收

- `cargo test --workspace` 全绿。
- `cargo xtask dashboard` 可重复执行。
- 核心 crate 不依赖 CLI 或 UI。

## 记录

先保持三个 crate。等 provider、tools、sandbox 真正落地后再拆，避免空 crate。
