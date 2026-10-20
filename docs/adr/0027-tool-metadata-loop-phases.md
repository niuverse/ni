+++
id = "adr-0027"
title = "ADR-0027：工具 risk/effect metadata 与 LoopPhase"
kind = "adr"
status = "accepted"
order = 54
summary = "内置工具声明 low/medium/high risk 与 read/write/execute/delegate effect；unknown 按 high 处理；loop 以 LoopPhase 事件暴露 project/request/stream/tool/turn/done。"
+++

## 状态

Accepted · 2026-10-20

## 决策

1. `niucode-tools` 提供静态 `ToolMetadata { risk, effect, read_only }`；未知/外部工具由调用方按 unknown/high 处理。
2. `/tools` 展示内置工具与已挂载 extension 的元数据，便于审计。
3. Agent loop 通过 `AgentEvent::Phase(LoopPhase)` 暴露 `project/request/stream/tool/turn/done`；UI 可选择消费或忽略。
4. metadata 先作为观察/授权输入，不替换现有 sandbox/approval；后续 auto-review 和 subagent 授权可复用。
5. LoopPhase 不写入 session 事件日志，避免每 turn 噪声；它是 observer seam。
