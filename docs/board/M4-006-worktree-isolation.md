+++
id = "M4-006"
title = "Backlog: Worker worktree 写隔离"
status = "backlog"
milestone = "M4"
module = "tools"
size = "M"
priority = "P2"
updated = "2026-10-20"
depends_on = ["M4-003"]
docs = ["adr-0014"]
+++

## 范围

- Worker 并行写时使用 git worktree / overlay，返回 patch 或合并结果。
- 非 git 仓库 fallback 到 W1 串行写。
- 处理 cwd/path 映射、测试缓存、清理和冲突。

## 验收

- [ ] 两个 Worker 可在独立 worktree 改不同模块。
- [ ] 主 session 能 review/apply 结果；失败可丢弃。
- [ ] 非 git 仓库仍可安全串行写。
