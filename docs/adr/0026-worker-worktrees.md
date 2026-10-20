+++
id = "adr-0026"
title = "ADR-0026：Worker 可选 git worktree 与 patch 回传"
kind = "adr"
status = "accepted"
order = 53
summary = "Worker 可声明 worktree=true，在全局 worktrees 下建 detached git worktree；写互不干扰，结束后产出 patch；主 session 显式 `niucode worktree apply`，非 git 回退共享 W1。"
+++

## 状态

Accepted · 2026-10-20

## 决策

1. `subagent` task 增加 `worktree: bool`，仅 Worker profile 可用。
2. worktree 根在全局 `~/.niucode/projects/<key>/worktrees/`；每个任务独立 detached worktree。
3. worktree 内 Worker 不共享主 workspace 的 W1 写锁，因为目录隔离；结束时 `git add -N .` + `git diff --binary` 生成 patch。
4. 结果摘要首行给出 patch 路径；worktree 移除，patch 保留。
5. 主 session 不自动改写工作区；用户 review 后显式 `niucode worktree apply --patch <path>`（可 `--check`）。
6. 非 git / 无 HEAD / worktree 创建失败时回退到共享 workspace + W1 串行写，并打印 warning。
7. 不做自动 merge/冲突解决；patch 是显式 review 边界。
