+++
id = "M5-003"
title = "Inline ratatui input layer"
status = "done"
milestone = "M5"
module = "tui"
size = "L"
priority = "P1"
updated = "2026-10-20"
completed = "2026-10-20"
depends_on = ["M5-002"]
docs = ["adr-0019", "tui"]
+++

## 范围

- ratatui inline viewport 的 picker/help、input、footer。
- transcript 保持原生 scrollback；不进入 alternate screen。
- picker 鼠标捕获；linear fallback。
- 迁移期 `NIUCODE_TUI_INLINE=1` opt-in。

## 验收

- [x] ratatui 0.30 + crossterm 0.28 backend 接入。
- [x] inline viewport 绘制 footer / input / picker。
- [x] cursor-position 不可用时自动回退 linear editor。
- [x] 多行输入（3 行可视）、Ctrl-J/Shift+Enter 换行。
- [x] 鼠标 picker hover/click；绝对坐标命中。
- [x] `@` 文件补全；根 `.gitignore` 基础过滤。
- [x] 默认切换；`NIUCODE_TUI_LINEAR=1` 仅作为终端能力 fallback。
- [x] PTY 终端模拟器验证 hover 行命中；单元测试覆盖 cursor/gitignore。

## 记录

- `inline.rs` 固定 8 行 viewport：5 行 picker/help + 2 行 input + 1 行 footer。
- 支持基本编辑、历史、slash/model/permissions picker；尚未完成鼠标、多行布局、Ctrl-R 完整行为。
- 默认 inline；cursor-position 不可用或 `NIUCODE_TUI_LINEAR=1` 时回退 `LineEditor`。
- PTY 终端模拟器验证 picker 行 hit-test；固定 8 行 viewport（4 picker/help + 3 input + 1 footer）。
- 已知边界：长行不横向滚动；嵌套 `.gitignore`/negation 未完整支持；TestBackend 快照未覆盖。
