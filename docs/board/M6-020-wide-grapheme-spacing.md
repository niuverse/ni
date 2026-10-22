+++
id = "M6-020"
title = "Wide graphemes no longer get filler spaces"
status = "done"
milestone = "M6"
module = "tui"
size = "S"
priority = "P1"
updated = "2026-10-22"
completed = "2026-10-22"
depends_on = ["M6-010"]
docs = ["adr-0028"]
+++

## 问题

Inline TUI 中，`HELLO 中文 WORLD` 实际输出为 `HELLO 中 文  WORLD`：Ratatui 的 `Paragraph` 在宽字符的第二个 cell 写入空格，后端又把该空格写出来，于是每个 CJK 字符后面多一列。

Linear 模式没有 `Paragraph`，同样的文本正常；这也解释了为什么只有 inline 画面显得「字间距太大」。

## 修复

- `collapse_wide_continuations` 在 `Paragraph` 渲染后，把宽字符占用的后续 cell 清成空字符串，后端就不再写填充空格。
- 应用于 transcript 的 `insert_before` 与 composer 的 `draw`。
- 单元测试断言 `中文` 渲染后 `(1,0)` / `(3,0)` 为空，`(0,0)` / `(2,0)` 是汉字。
- PTY 抓取确认 raw bytes 为 `HELLO 中文 WORLD`，不再有中间空格。

## 验收

- [x] Inline transcript 的 CJK 字符之间无填充空格。
- [x] Composer 的宽字符走同一修复。
- [x] Linear 输出不变。
- [x] workspace tests、fmt、clippy、dashboard 通过。
