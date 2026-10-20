+++
id = "M5-002"
title = "Global per-project workspace and session index"
status = "done"
milestone = "M5"
module = "workspace"
size = "M"
priority = "P1"
updated = "2026-10-20"
completed = "2026-10-20"
depends_on = ["M5-001"]
docs = ["adr-0018", "tui", "exec"]
+++

## 范围

- `~/.niucode/projects/<key>/`：sessions / proposals / backups / extensions / cache / worktrees。
- 项目根零 `.niucode`；旧路径硬切。
- `project.toml` canonical cwd / first_seen / last_seen。
- session 标题从首条 user message 派生；`/resume` 当前项目，`/resume --all` 跨项目。
- `config.toml` 默认 provider/model/context，CLI 覆盖。
- `niucode gc` dry-run；显式清理。
- ToolResult 1 MiB 上限。

## 验收

- [x] 工作区不创建项目 `.niucode`；project-key 稳定。
- [x] sessions 写入项目目录；旧 flat sessions 不自动列出。
- [x] `/resume` 显示首条 prompt / provider / model / 相对时间；`--all` 跨项目。
- [x] `config.toml` provider/model/context 默认生效，显式 CLI 优先。
- [x] `niucode gc` 默认只读；`--apply` 清 cache/旧 backups/proposals；session 需 `--prune-sessions --yes`。
- [x] ToolResult 1 MiB head/tail 截断。
- [x] 全局 extensions 目录挂载；项目 `.niucode/extensions` 不再读取。

## 记录

- `workspace.rs` 负责 global root、project key、project.toml、目录布局。
- `session_summary` 读首条 user_message；`/resume` 最近 10 个、`--all`。
- `niucode gc` 默认 dry-run；`--apply` 保留最近 5 backups / 10 proposals，清 cache。
- `cap_tool_output` 1 MiB；单测覆盖 head/tail。
- 旧 sessions/extensions 不迁移；用户需手工处理。
- `/resume` 仍是文本列表，交互式 picker 留到 TUI inline 重写。
