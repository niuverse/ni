+++
id = "M0-008"
title = "Turn loop 与 ni exec"
status = "backlog"
milestone = "M0"
module = "loop"
size = "L"
priority = "P0"
updated = "2026-10-20"
depends_on = ["M0-005", "M0-006", "M0-007"]
docs = ["architecture"]
+++

## 范围

- 一个 turn：project → stream → dispatch tools → append events。
- 循环直到无 tool call、达到 max steps、被取消或 fatal error。
- 只读工具可并发，写工具串行。
- `ni exec "<prompt>"` headless 入口，输出最终文本和 usage。

## 验收

- 用 scripted provider 修掉 fixture 仓库里的一个失败测试。
- 达到 max steps 时返回明确原因，不静默退出。
- 取消后不再发起 provider 请求或工具调用。
- 每个 turn 的 usage 落在 `turn_end` 事件里。

## 记录

loop 不关心 provider wire format，也不实现工具逻辑。它只编排和记录。
