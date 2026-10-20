+++
id = "M5-003"
title = "Inline ratatui input layer"
status = "review"
milestone = "M5"
module = "tui"
size = "L"
priority = "P1"
updated = "2026-10-20"
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
- [ ] 多行输入、鼠标 picker hover/click、`@` 文件补全与 linear parity。
- [ ] 默认切换并删除长期双轨。
- [ ] ratatui TestBackend / PTY 快照覆盖。

## 记录

- `inline.rs` 固定 8 行 viewport：5 行 picker/help + 2 行 input + 1 行 footer。
- 支持基本编辑、历史、slash/model/permissions picker；尚未完成鼠标、多行布局、Ctrl-R 完整行为。
- `NIUCODE_TUI_INLINE=1` 启用；失败回退 `LineEditor`。
- 默认仍为 linear，避免未 parity 的布局影响日常使用。
