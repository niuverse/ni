+++
id = "M6-023"
title = "Embedder command hook"
status = "done"
milestone = "M6"
module = "tui"
size = "S"
priority = "P1"
updated = "2026-10-22"
completed = "2026-10-22"
depends_on = ["M6-022"]
docs = ["adr-0032"]
+++

## 范围

- `TurnHooks::command(&mut self, input: &str) -> Option<String>` 默认 `None`。
- TUI 的 `SlashParsed::Unknown` 先交给 hook；返回文本则打印，`None` 保留原来的 unknown 提示。
- example 探针对 `/probe` 返回 `probe-ok`，PTY 验证输出与 `command` 记录。
- 这是 embedder 命令注册的第一步；help/completion 还没有接入。

## 验收

- [x] 未知 slash 命令可以交给 embedder。
- [x] 返回文本由 TUI 打印。
- [x] 未处理时保留 unknown 提示。
- [x] PTY 运行时验证。
