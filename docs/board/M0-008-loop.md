+++
id = "M0-008"
title = "Turn loop 与 niucode exec"
status = "done"
milestone = "M0"
module = "loop"
size = "L"
priority = "P0"
updated = "2026-10-20"
completed = "2026-10-20"
depends_on = ["M0-005", "M0-006", "M0-007"]
docs = ["architecture", "exec"]
+++

## 范围

- 一个 turn：project → stream → dispatch tools → append events。
- 当前是函数边界 `run(provider, session, config, prompt, observer)`；Command/Event 通道留给 TUI/Web 阶段（ADR-0004）。
- 循环直到无 tool call、达到 max steps、被取消或 fatal error。
- 只读工具可并发，写工具串行。
- `niucode exec "<prompt>"` headless 入口，输出最终文本和 usage。

## 验收

- [x] 用 scripted provider 修掉 fixture 仓库里的一个失败测试（read → edit → bash → 汇总）。
- [x] 达到 max steps 时返回明确原因，不静默退出。
- [x] Ctrl-C 取消后 drop provider stream / 工具 future，进程组被清理。
- [x] 每个 turn 的 usage 落在 `turn_end` 事件里。

## 记录

loop 不关心 provider wire format，也不实现工具逻辑。它只编排和记录。当前没有 Command/Event 队列；TUI/Web 接入时再包一层 SessionRuntime。
