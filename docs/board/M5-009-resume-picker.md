+++
id = "M5-009"
title = "Interactive /resume picker"
status = "done"
milestone = "M5"
module = "tui"
size = "S"
priority = "P1"
updated = "2026-10-20"
completed = "2026-10-20"
depends_on = ["M5-002"]
docs = ["tui"]
+++

## 范围

- `/resume` 无参数时进入上下选择列表，而不是只打印文本。
- 非 TTY 保留文本列表。
- 临时 alternate screen，退出恢复 transcript。

## 验收

- [x] ↑↓/j/k 选择，Enter 恢复，Esc 取消。
- [x] 标题/provider/model/相对时间。
- [x] `/resume --all` 仍跨项目。
- [x] 非 TTY 回退文本列表。
- [x] tmux 抓屏验证列表对齐（raw mode 下使用 `\r\n`）。

## 记录

- picker 使用 alternate screen；不污染 transcript 光标。
- 非 TTY 路径不做交互。
