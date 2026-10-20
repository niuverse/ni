+++
id = "M0-009"
title = "Resume 与录制回放测试"
status = "done"
milestone = "M0"
module = "session"
size = "M"
priority = "P1"
updated = "2026-10-20"
completed = "2026-10-20"
depends_on = ["M0-008"]
docs = ["adr-0002", "exec"]
+++

## 范围

- `ni exec --resume <session>` 从日志重建上下文继续。
- scripted provider：按预设返回 tool call 序列，完全离线。
- 录制 / 回放 provider 响应的 fixture 目录。
- golden transcript 测试 loop 行为。

## 验收

- [x] `ni exec --resume <session>` 从日志重建上下文继续；未指定 `--cwd` 时使用 session 的 cwd。
- [x] `--record` 把归一化 provider turn 写成 fixture；`--replay` 完全离线重放，不需要 key。
- [x] 回放同一 fixture 两次，事件序列一致。
- [x] golden transcript 测试不访问网络、不烧 token。
- [x] 工具执行后 resume 不会重放已完成 turn。

## 记录

实现 `project_history`、`SessionLog::open`、`RecordingProvider` / `ReplayProvider` 和 golden fixture。测试的价值不在覆盖率，而在让 context / loop 的修改可以离线验证。
