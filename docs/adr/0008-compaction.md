+++
id = "adr-0008"
title = "ADR-0008：Compaction 是事件日志上的确定性 handoff"
kind = "adr"
status = "accepted"
order = 27
summary = "75% 预算触发；保留最近 N 轮，其余事件生成结构化 handoff；upto 是绝对事件下标，日志不重写。"
+++

## 状态

Accepted · 2026-10-20

## 背景

M1 结束时，loop 每轮把完整历史发给 provider。日志可以一直增长，但模型上下文不能。`Compaction` 事件已经在 core 里，但 `upto` 没有被投影使用，也没有触发、handoff 和回放规则。

需要同时满足三件事：

- 长会话要能继续，不能因为上下文上限直接失败。
- 压缩必须可回放：同一份日志在 resume 后得到与压缩时一致的模型上下文。
- 日志是唯一真相，压缩不能删除或改写旧事件。

另一个现实约束：当前没有 tokenizer，也没有额外的摘要模型调用。M2-001 先用确定性规则和近似 token 预算，把协议与回放做对。

## 决策

1. **Context 属于 Engine，不属于 core。** 投影和压缩放在 `crates/niucode-agent/src/context.rs`；`niucode-core` 继续只拥有 `Event` 和 `SessionLog`。这样 core 不依赖 provider 的 `ChatMessage`。
2. **预算用近似 token。** ASCII 约 4 字符 1 token，非 ASCII 1 字符 1 token；加消息和工具 schema 的固定开销。默认预算 64k，到 75% 触发；CLI 暴露 `--context-budget` 和 `--keep-recent-turns`。
3. **M2-001 的 handoff 是确定性的，不额外调用模型。** 从日志生成目标、上一次请求、工具动作、成功写入的文件、最近助手状态、近期失败、约束和下一步。结构稳定，离线可测。
4. **`compaction.upto` 是绝对事件下标。** 摘要替换 `events[..upto]`；compaction 事件本身追加在日志末尾，但投影时在 `upto` 处插入摘要，而不是在事件物理位置插入。最后一个 compaction 生效，旧 compaction 被忽略。
5. **保留最近 N 个 user turn 原文。** 多轮会话以第 N 个最近的 user message 为 cutoff。单个 turn 很长时，回退到该 turn 内安全的 assistant 边界，保留尾部，避免 tool call 和 tool result 被切开。
6. **压缩成功才落事件。** 先构造候选投影并估算 after；只有 after 小于 before 才追加 `compaction`。追加失败或不收缩时保留原上下文，并通过 `Warning` 事件提示。
7. **Token 计数是审计元数据。** `compaction` 事件新增 `before_tokens` / `after_tokens`，带 `serde(default)`，旧日志可读。

## 后果

### 正向

- 压缩与 resume 共用 `project_history`，不会出现 UI 和 loop 各自维护一份摘要。
- 日志保留全部原始事件，压缩事件只改变投影。
- 确定性 handoff 可以离线回放，不消耗额外 token，不会因为摘要模型失败而丢历史。
- 单 turn 长任务也有压缩路径，不会因为只有一条 user message 卡死。
- 预算、cutoff、token 估算都是显式配置，测试可以设得很小。

### 负向

- 确定性 handoff 不如模型摘要擅长语义压缩；复杂任务可能丢掉细微意图。
- token 估算是启发式，可能过早或过晚触发；真实 usage 仍是账单依据。
- 单 turn 回退只能压缩 assistant 边界之前的内容；如果最新工具输出本身就是大头，压缩可能不收缩，此时会提示并保留上下文。
- 每次检查都从 JSONL 重新读日志；长会话的压缩检查有文件 I/O 成本。

## 备选方案

- **模型生成摘要**：语义更好，但压缩本身要发一次请求，引入失败、成本和新的取消路径；M2-001 先不采用，等确定性 handoff 的质量边界清楚后再加可选模式。
- **只截断旧消息**：实现最简单，但会丢掉原始目标和已完成工作，agent 容易重复劳动。
- **改写或删除旧事件**：违反“日志是唯一真相”，不做。
- **在 core 里做投影**：会让 core 依赖 provider 的消息类型，破坏依赖方向。
- **用精确 tokenizer**：需要模型相关依赖和版本维护；先保留估算接口，后续可替换。

## Revisit 条件

1. 确定性 handoff 在真实长任务中反复丢意图，需要模型生成或人可编辑的 handoff。
2. 引入与模型匹配的 tokenizer，或 provider 返回更精确的上下文预算。
3. 压缩检查的 JSONL 读取成为性能瓶颈，需要内存事件索引。
4. 出现多文件 patch、subagent 或跨 session 合并，handoff 需要新的结构。
