+++
id = "M4-006"
title = "Worker worktree write isolation"
status = "done"
milestone = "M4"
module = "tools"
size = "M"
priority = "P2"
updated = "2026-10-20"
completed = "2026-10-20"
depends_on = ["M4-003"]
docs = ["adr-0026", "tui"]
+++

## 范围

- Worker `worktree=true` 使用全局 project worktrees 下的 detached git worktree。
- 结束后生成 patch，主 session review/apply。
- 非 git fallback 到共享 workspace + W1。

## 验收

- [x] 两个 Worker worktree 独立，patch 分别包含各自修改。
- [x] 主 workspace 不被 Worker 直接修改。
- [x] patch 路径写入结果摘要；`niucode worktree apply --patch` 可 check/apply。
- [x] 非 git workspace 回退共享 workdir（返回 None）。
- [x] worktree 清理。

## 记录

- 核心单测：两个 worktree、patch、parent 不变、非 git fallback。
- `subagent` schema 增加 `worktree`；仅 Worker 可用。
- 未做：自动 merge/冲突处理、worktree 内 extension host 单独挂载、patch 二进制审查 UI。
