+++
id = "M4-003"
title = "Subagent Explore/Worker capability profiles"
status = "done"
milestone = "M4"
module = "loop"
size = "M"
priority = "P2"
updated = "2026-10-20"
completed = "2026-10-20"
depends_on = ["M4-002"]
docs = ["adr-0014", "adr-0011", "exec"]
+++

## 范围

- Explore：read-only + read/grep/glob + 只读 external tools。
- Worker：workspace + read/write/edit/grep/glob/bash + 只读 external tools。
- W1：父/子共享一个 write lock；write/edit 支持 `write_scope`。
- subagent 不默认 full-access；mutating external tools 不进子 agent。
- tool schema 来自 registry view，不复制 host 进程。

## 验收

- [x] Explore/Worker 的 tool view 不同且可审计（spawn 事件记录 profile）。
- [x] Worker 写操作共享全局写锁；write/edit path scope 生效。
- [x] 多个 subagent 共享 session extension host，只读外部工具可用。
- [x] Explore 不能声明 `write_scope`，也不能看到 write/bash。

## 记录

- `subagent` schema 增加 `profile: explore|worker`（默认 explore）和 Worker 的 `write_scope`。
- `SubagentSpawn` 增加 `profile`（serde 默认，兼容旧日志）。
- Explore：sandbox read-only/network off；allowlist `read/grep/glob` + `host.read_only_tools()`。
- Worker：继承父 sandbox；allowlist 加 `write/edit/bash` + `host.read_only_tools()`；`write_scope` 校验 write/edit 路径。
- W1：`AgentConfig.write_lock: Option<Arc<tokio::sync::Mutex<()>>>`，CLI/TUI 创建，父与 Worker 共享；dispatch 在 write/edit/bash 前持有。
- sidecar manifest 增加 `read_only`（默认 true）；`ExtensionHost::read_only_tools()` 只暴露只读插件的工具给子 agent。
- 限制：bash 不受 `write_scope` 约束，只靠全局锁串行；Worker 在父 `approval=ask` 下写会被子 agent 的拒绝式 approver 拒绝（安全默认）。mutating external tool 仍只属于主 session。
- 测试：Worker tool view/profile 事件、Explore + Python 只读插件工具、write_scope 前缀单测、原有并行/超时/错误隔离。
