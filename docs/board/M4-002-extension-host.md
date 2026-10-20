+++
id = "M4-002"
title = "ExtensionHost v1 与 external tools"
status = "done"
milestone = "M4"
module = "extensions"
size = "L"
priority = "P1"
updated = "2026-10-20"
completed = "2026-10-20"
depends_on = ["M4-001"]
docs = ["adr-0013", "exec"]
+++

## 范围

- session 级 ExtensionHost：`<workspace>/.niucode/extensions/*.toml` 发现 manifest。
- 一插件一 sidecar 进程；`initialize` / `tools/call` / `shutdown` 的 line-delimited JSON-RPC。
- sidecar 环境变量清空（只保留解释器启动所需），不继承 API key。
- 每插件调用串行；超时/崩溃返回 tool error，不 panic 父 session。
- 父 session 追加 `extension_mount` / `extension_unmount` / `extension_error`。

## 验收

- [x] 一个 Python sidecar 注册只读工具并可从主 session 调用。
- [x] 插件超时/错误返回 tool error，不影响父 session 继续。
- [x] 父 session 记录 mount/unmount provenance；工具结果走正常事件日志。
- [x] sidecar 不继承凭证类环境变量。

## 记录

- 新 crate `niucode-extension`：manifest 解析、spawn、JSON-RPC 请求/响应、工具注册、超时、shutdown。
- `niucode-agent`：`AgentConfig.extension_host` 合并 specs；dispatch 在 built-ins/subagent 之后调用 host。
- `niucode exec` / `niucode tui`：启动时挂载，结束时 unmount；事件写入父 session。
- 测试：Python sidecar 注册/调用、built-in 重名拒绝、凭证 env 不泄漏、timeout 隔离、agent loop 调用 external tool、exec 端到端。
- v1 是**可信本地 sidecar**：没有 worker pool、没有 bwrap 网络/能力隔离、没有 subagent 共享 host；这些统一进 M4-007。
