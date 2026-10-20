+++
id = "M0-009"
title = "Resume 与录制回放测试"
status = "backlog"
milestone = "M0"
module = "session"
size = "M"
priority = "P1"
updated = "2026-10-20"
depends_on = ["M0-008"]
docs = ["adr-0002"]
+++

## 范围

- `ni exec --resume <session>` 从日志重建上下文继续。
- scripted provider：按预设返回 tool call 序列，完全离线。
- 录制 / 回放 provider 响应的 fixture 目录。
- golden transcript 测试 loop 行为。

## 验收

- 中断后 resume，模型看到的历史与中断前一致。
- 回放同一 fixture 两次，事件序列一致。
- 工具执行后不重放整个 turn。
- 测试不访问网络、不烧 token。

## 记录

测试的价值不在覆盖率，而在让 context / loop 的修改可以离线验证。
