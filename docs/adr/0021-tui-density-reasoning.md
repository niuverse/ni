+++
id = "adr-0021"
title = "ADR-0021：默认紧凑 inline 区，reasoning 可见"
kind = "adr"
status = "accepted"
order = 48
summary = "无 picker 时底部占 3 行；打开 picker 才扩到 6 行；reasoning 流以 dim 文本实时显示，不再只显示 thinking 标签。"
+++

## 状态

Accepted · 2026-10-20

## 背景

inline v1 固定 8 行（4 help/picker + 3 input + footer），即使没有 picker 也长期占满底部；help 文案重复，输入区显得拥挤。reasoning 事件只打印一次 “thinking…”，用户看不到实际思考内容，主观上像被隐藏。

## 决策

1. 输入区高度动态：
   - 无 picker：3 行（2 行输入 + 1 行 footer）；
   - 打开 picker：6 行（3 行候选 + 2 行输入 + footer）；
   - 通过重建 inline viewport 实现，关闭 picker 后回到 3 行。
2. 移除常驻 help 墙；slash/@ 提示收进 footer。
3. reasoning 以 `◇ thinking` 标签 + dim 流式文本显示；保留换行，不把 reasoning 拼成一行。
4. answer/tool/compaction 开始时关闭 reasoning 行，避免串行。
5. 不默认隐藏 reasoning；后续需要时再加 `/reasoning` 开关。

## 后果

- 默认 transcript 获得更多空间，底部不再长期拥挤。
- picker 展开有一次性 viewport 重建，但不在 Drop 里做 cursor 跳转；tmux 抓屏验证提交前后无覆盖。
- reasoning 会增加屏幕输出；长 reasoning 仍完整显示，暂不做折叠。
