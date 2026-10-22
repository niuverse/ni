+++
id = "M6-025"
title = "Register embedder commands in help and completion"
status = "done"
milestone = "M6"
module = "tui"
size = "S"
priority = "P1"
updated = "2026-10-22"
completed = "2026-10-22"
depends_on = ["M6-023"]
docs = ["adr-0032"]
+++

## 范围

- `TurnHooks::commands` 默认空列表。
- `CommandRegistry::extend` 追加命令、补 `/`、跳过与内建重名。
- TUI 启动时把 hook 命令并入 registry；`/help` 与 Tab completion 共用。
- example 注册 `/probe`，PTY 验证 `/help` 出现且 `/probe` 仍由 command hook 执行。

## 验收

- [x] 自定义命令出现在 `/help`。
- [x] completion 候选包含自定义命令。
- [x] 内建命令不被覆盖。
- [x] PTY 验证 `/probe` 输出与注册。
