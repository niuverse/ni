+++
id = "M5-005"
title = "Compact inline area and visible reasoning"
status = "done"
milestone = "M5"
module = "tui"
size = "S"
priority = "P1"
updated = "2026-10-20"
completed = "2026-10-20"
depends_on = ["M5-003"]
docs = ["adr-0021", "tui"]
+++

## 范围

- 默认底部区固定 3 行（输入 2 + footer）。
- picker 打开时扩到 6 行，关闭恢复。
- reasoning 实时显示，而不是只显示 thinking 标签。

## 验收

- [x] 无 picker 时 `2 input + 1 footer`。
- [x] picker 打开时 `3 picker + 2 input + footer`。
- [x] `ReasoningDelta` 文本可见、保留换行。
- [x] answer/tool 开始前关闭 reasoning 行。
- [x] PTY + replay fixture 验证 reasoning 文本和 answer 都出现。

## 记录

- inline viewport 重建成 3/6 行；移除常驻 help 墙；不再在 Drop 里 MoveTo。
- `Think` 标签保留，实际 reasoning 以 dim 流输出。
- 未做 reasoning 折叠/开关；长 reasoning 会占用 transcript。
