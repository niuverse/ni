+++
id = "M4-003"
title = "Subagent Explore/Worker capability profiles"
status = "backlog"
milestone = "M4"
module = "loop"
size = "M"
priority = "P2"
updated = "2026-10-20"
depends_on = ["M4-002"]
docs = ["adr-0014", "adr-0011"]
+++

## 范围

- Explore：read-only + 只读插件 allowlist。
- Worker：workspace + write/edit/bash + 只读插件 allowlist；W1 全局写锁 + path scope。
- subagent 不默认 full-access；高风险插件工具仅在主 session。
- tool schema 来自 registry view，不复制 host 进程。

## 验收

- [ ] Explore/Worker 的 tool view 不同且可审计。
- [ ] Worker 写操作串行，path scope 生效。
- [ ] 多个 subagent 共享 host，进程数不随 subagent 增长。
