+++
id = "M6-024"
title = "Post-turn prompt hook"
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

- `TurnPrompt { id, question, options }`。
- `TurnHooks::after_turn` 在成功 turn 后返回问题列表；`on_answer(id, zero_based)` 接收选择。
- TUI 打印编号问题，在 TTY 下读取 1..N；非 TTY 只打印，不阻塞。
- example 探针返回两选项问题，PTY 输入 `1`，记录 `answer:probe:0`。
- 这是 candidate 确认的通用 seam，不把身份/记忆类型带进 niucode。

## 验收

- [x] 成功 turn 后调用 after_turn。
- [x] 选项编号与零基 on_answer 对应。
- [x] 非 TTY 不阻塞。
- [x] PTY 运行时验证。
- [x] 默认 NoHooks 不受影响。
