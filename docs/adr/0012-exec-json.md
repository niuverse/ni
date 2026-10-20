+++
id = "adr-0012"
title = "ADR-0012：exec --json 是稳定的 JSONL 协议"
kind = "adr"
status = "accepted"
order = 31
summary = "每行一个 JSON 事件；start 带 protocol=1；stdout 只放协议，stderr 放人类诊断；approval=ask 被拒绝。"
+++

## 状态

Accepted · 2026-10-20

## 背景

M3 的退出条件之一是 `exec --json` 成为稳定协议。现有 `niucode exec` 把模型文本写 stdout、状态写 stderr，适合人读，不适合脚本、editor 集成或 CI 判断。需要一个机器可读、增量流式、不会把完整工具输出重复塞进协议的接口。

## 决策

1. **stdout 是 JSONL，一行一个 JSON 对象。** stderr 继续放人类诊断（启动信息、错误、session 路径）；stdout 不混入普通文本。
2. **`start` 带 `protocol: 1`。** 客户端先读第一行，确认 `type = "start"` 和协议版本；后续只做加法兼容，不改变已有字段含义。
3. **事件类型固定为：**
   - `start`：session、model、provider、cwd、sandbox、network、approval、backend。
   - `text_delta` / `reasoning_delta`：增量文本。
   - `tool_call`：name、input。
   - `tool_result`：name、ok、summary。**不包含完整 output**，完整内容在 session JSONL。
   - `turn_end`：usage（input/output/cached/reasoning/cache percent）。
   - `compaction`：replaced_events、before/after token 估算。
   - `warning`：message。
   - `done`：turns、usage、session。
   - `cancelled` / `error`：终止状态；error 同时以非零退出码结束。
4. **`--json` 不交互。** `--approval ask` 直接拒绝；如果没有 bwrap 导致 `on-failure` 降级为 ask，也拒绝。防御性地，如果 JSON 模式下真的收到审批请求，输出一条 warning 并拒绝该工具，而不是阻塞读 stdin。
5. **退出码保持 shell 语义。** 成功和用户取消返回 0；模型/provider 错误返回非零，stdout 仍以 `error` 事件结束。
6. **协议不重复 session 日志。** `tool_result` 只给 summary；需要完整工具输出、reasoning 或事件历史时读 `session` 路径。这样 JSONL 的每行大小可控，也避免第二份真相。
7. **`--json` 只属于 headless exec。** TUI 是交互界面，不提供该模式。

## 后果

### 正向

- 脚本和编辑器可以增量消费同一个事件流，不需要解析人类输出。
- 协议行大小可控；完整 output 在日志里，仍可追溯。
- 与 `exec --record` / `--replay` 兼容，离线测试不需要 API key。
- 事件类型与 `AgentEvent` 基本同构，CLI/TUI/JSON 三种 client 共享语义。

### 负向

- 新增一个需要维护的对外 schema；字段只能加法演进。
- `tool_result` 不含完整 output，客户端需要额外读 session 文件。
- `--approval ask` 不可用，交互式审批只能走 TUI。
- stderr 仍是人类文本，纯机器消费者需要忽略它。

## 备选方案

- **一次性 JSON 数组**：简单，但不是流式，长任务无法增量消费。
- **NDJSON 中包含完整 tool output**：信息全，但 bash 20KB 输出会撑大每一行，且与 session 日志重复。
- **复用 provider 原始 SSE**：泄漏 wire format，破坏 provider 薄边界。
- **单独 metrics 文件**：又一份持久状态，和事件日志重复。
- **TUI 也支持 --json**：TUI 本身是交互界面，混合模式会增加终端状态复杂度，不做。

## Revisit 条件

1. 需要 editor/LSP 双向协议，而不只是单向事件流。
2. `tool_result` 的 summary 不够，需要分页或按需拉取完整 output。
3. 协议需要多版本并存，引入 `protocol: 2` 和协商字段。
4. 需要非交互审批（例如 policy server），引入审批事件/回调而不是 stdin。
