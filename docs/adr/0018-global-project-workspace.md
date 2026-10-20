+++
id = "adr-0018"
title = "ADR-0018：全局 per-project 工作区，项目根零状态"
kind = "adr"
status = "accepted"
order = 45
summary = "所有 niucode 状态在 ~/.niucode/projects/<key>/；项目根不创建 .niucode；session 按项目分组；旧路径硬切不读；默认不自动回收。"
+++

## 状态

Accepted · 2026-10-20

## 背景

项目级 `.niucode` 会让每个仓库变脏，尤其 sessions/cache/proposals 这类运行时状态。Codex 式工作区更适合放在用户 home；项目根只保留 `AGENTS.md` 和用户自己创建的 `.agents/skills`。

## 决策

1. **全局根 `~/.niucode/`**；`NIUCODE_HOME` 可覆盖。
2. 项目目录 `~/.niucode/projects/<basename>-<fnv-hash>/`，`project.toml` 记录 canonical cwd / first_seen / last_seen。
3. 项目根**不创建** `.niucode`；扩展、sessions、proposals、backups、cache、worktrees 都在全局项目目录。
4. session 默认只列当前项目；`/resume --all` 跨项目。标题从第一条 `user_message` 派生，回退 goal / session id。
5. 默认**不自动删除**任何数据；`niucode gc` 默认 dry-run，`--apply` 只清派生数据，`--prune-sessions --yes` 才碰 session。
6. 单条 ToolResult 上限 1 MiB（head/tail + marker），避免单次输出撑爆 session。
7. 旧 `~/.niucode/sessions/*.jsonl` 与项目 `.niucode/extensions` **硬切不读**；不提供隐式迁移。
8. `~/.niucode/config.toml` 只放非敏感默认值 provider/model/context；CLI 显式参数优先，凭据仍走 auth store。

## 后果

- 项目干净，session 天然按项目隔离。
- 项目移动会生成新 project-key；旧 session 仍可按路径显式 resume，但不自动聚合。
- 没有自动 GC；用户需要看 `niucode gc` 并显式清理。
- 团队无法通过 git 提交项目扩展；未来若要版本化，走显式导出而不是自动创建项目 `.niucode`。
