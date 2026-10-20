+++
id = "adr-0013"
title = "ADR-0013：小内核 + 外置 capability 插件"
kind = "adr"
status = "accepted"
order = 40
summary = "内置工具、loop、context、policy、auth、session 留在内核；external tools/commands 走 session 级 sidecar host；WASM 只做纯计算 tier。"
+++

## 状态

Accepted · 2026-10-20

## 背景

M4 要让 niucode 可扩展，同时不牺牲事件日志、replay、安全和可观察性。Pi 的同进程 TypeScript extension 和 DSH/Cordis 的 everything-is-plugin 都提供了很好的扩展点，但一个把信任边界交给同进程，一个把整个 runtime 变成插件树。niucode 的体量不需要平台级 runtime，但需要能力级扩展。

## 决策

1. **内核保持特权且小**：`niucode-core` 的 Event/SessionLog、context 投影与 compaction、turn loop、tool dispatch、subagent 编排、policy/sandbox/approval、auth、内置工具、extension supervisor 都不插件化。
2. **只插件化两类代码**：external tools 和 plugin commands。skills/AGENTS/command metadata/provider presets 是声明式扩展，不是代码插件。
3. **内置工具留在内核**：它们定义最小闭环、安全边界和 subagent allowlist；外部工具走同一 `Tool` 接口，但属于低信任 tier。
4. **session 级 host**：一个 session 一个 `ExtensionHost`；一个插件一个 sidecar 进程（可带 worker pool）。进程数按插件计，不按 subagent 计；subagent 共享 host 和 provider pool。
5. **transport 用 stdio JSON-RPC**：manifest + `initialize` 握手 + capability negotiation + `tools/call` / `commands/call` + `shutdown`；语言无关，agent 可用 Python/JS/Rust 写插件。
6. **生命周期在 turn 边界**：`DISCOVERED → VALIDATED → MOUNTED → RUNNING → STOPPING → STOPPED/FAILED`；工具 schema 来自 manifest，进程可懒启动；注册项带 generation，可逆回收；超时/取消 kill 进程组。
7. **能力边界**：插件默认无凭证、无网络、只能通过宿主能力访问文件/网络/子进程；manifest 声明 capabilities；高风险能力需要显式授权。
8. **WASM 是第二 tier**：只用于纯计算/不可信 transform（parser、linter、policy predicate、codec）；AOT 缓存、fuel/memory 限制；不要求所有插件 WASM。
9. **事件 provenance**：父 session 追加 `extension_mount` / `extension_unmount` / `extension_error`，包含 id/version/digest/capabilities；子 session 记录自己的 tool calls；resume 对比 digest。
10. **不做**：plugin-owned event schema、plugin-owned durable state、plugin loop、plugin UI 直接操作终端、递归 subagent 插件。

## Implementation note

M4-002 v1 先落本地可信 sidecar：manifest、initialize、tools/call、shutdown、timeout、env scrub 和 mount events。worker pool、bwrap 网络/能力隔离、subagent 共享 host 统一放 M4-007，避免把未实现的能力写成已完成。

## 后果

### 正向

- 扩展能力（内部 API、数据库、MCP、搜索、部署）不必进内核。
- 插件崩溃/超时不会带走父 session；凭证不进入插件进程。
- 内置工具无需插件即可工作；subagent 高并发不复制插件进程。
- 事件日志仍是唯一持久真相；插件只产生 tool result/audit。

### 负向

- IPC、manifest、supervisor、capability broker 都是新代码。
- 插件进程有内存/启动开销；需要 lazy start、worker pool、idle timeout。
- 外部 mutating tool 没有内置 write/edit 的 diff 审批；必须按 capability/effect 分级。
- 插件工具描述会占用 prompt；需要 allowlist 和数量上限。

## 备选方案

- **Pi 式同进程 TS**：DX 最好，但 Rust 内核无法安全卸载，插件可触达凭证和进程内存。
- **Cordis/DSH 式 everything-plugin**：生命周期最强，但 100+ 包、配置/事件契约和平台复杂度不适合当前体量。
- **WASM-first**：隔离强，但 OS/网络/子进程能力弱，agent 写 Rust/WASM 迭代慢。
- **每 subagent 一套插件**：进程数 S×P，直接拒绝。

## Revisit 条件

1. 需要插件在 turn 内动态注册/注销，或需要 loop/context 级 hook。
2. 外部 mutating tool 成为主流需求，需要统一 side-effect 审批/回滚。
3. 插件数量/并发超过进程模型承受范围，引入 worker pool 调度或 WASM compute tier。
4. 需要远程 sandbox（E2B/AgentENV 式）时，ExtensionHost 增加 proxy 模式。
