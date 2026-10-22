+++
id = "M6-022"
title = "Text delta filter hook"
status = "done"
milestone = "M6"
module = "tui"
size = "S"
priority = "P1"
updated = "2026-10-22"
completed = "2026-10-22"
depends_on = ["M6-021"]
docs = ["adr-0032"]
+++

## 范围

- `TurnHooks::filter_text(&mut String)` 默认空操作。
- exec、TUI live、无编辑器路径都在显示前调用它；持久化事件不变。
- example 探针把 `RAW` 替换为 `FILTERED`，PTY 验证 transcript 不含 raw，且 filter/turn_end 都触发。
- 这是 niuse 协议块剥离、renderer 复用的第一个 seam。

## 验收

- [x] 三条路径都调用 filter。
- [x] 事件日志未被修改。
- [x] PTY 运行时验证 filter 生效。
- [x] 默认 `NoHooks` 行为不变。
