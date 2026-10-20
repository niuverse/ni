+++
id = "adr-0010"
title = "ADR-0010：事件 ID 用稳定序号，fork 复制前缀并追加溯源事件"
kind = "adr"
status = "accepted"
order = 29
summary = "事件 ID 是解码事件的 1-based 序号；replay 只做投影；fork 复制前缀并在末尾追加 fork 溯源事件。"
+++

## 状态

Accepted · 2026-10-20

## 背景

M3-001 需要 `niucode replay <session>` 和 `niucode fork <session> --at <event-id>`。现有日志只有事件内容和时间戳，没有事件 ID；如果给每条事件加 UUID，会改动所有构造点和旧日志，而且回放不再确定。

日志是 append-only 的，因此“第几个解码事件”本身就是一个稳定标识。`load` 已经能跳过坏行并报告 warning；这正好可以作为 ID 的语义基础。

## 决策

1. **事件 ID 是解码事件的 1-based 序号。** 显示为 `e000012`，解析接受 `e12`、`E12` 或 `12`；`e000000` 非法。ID 不写进事件内容。
2. **坏行不制造 ID。** `load` 只把成功解码的事件计入序号，因此坏行不会让后续有效事件的 ID 漂移；这也意味着 ID 是“有效事件序号”，不是物理行号。
3. **fork 复制已验证的前缀。** `fork --at <id>` 解析 ID、校验范围，复制 `events[0..=index]` 到新 session，再在末尾追加一条 `fork` 事件：
   ```text
   Fork { parent, parent_event, ts }
   ```
   新文件通过 `SessionLog::create` 的 `create_new` 创建，不覆盖已有文件。源文件只读，不会被修改。复制的 `session_start` 保留父 session 的 id；新 session 的身份由文件名和末尾的 `fork` 事件表达，避免重写前缀。
4. **复制保持事件顺序和下标。** `compaction.upto` 是绝对事件下标，fork 不插入事件到前缀中，只在末尾追加溯源事件，因此旧 compaction 仍然有效。
5. **replay 只做投影。** 对每个 `user_message`，投影到“下一个 assistant 消息之前”，把 compaction / instructions 等请求前追加的事件也算进去；不构造 provider，不调用模型。每 turn 输出消息数、估算 token 和投影 digest。
6. **冲突/错误处理。** 无法解析、越界或指向不存在事件的 ID 直接失败；目标文件名冲突由 `create_new` 拒绝；坏尾行作为 warning 输出，fork 只复制有效前缀。

## 后果

### 正向

- 不改变事件 schema，旧日志可以直接 replay/fork。
- ID 在 append-only 日志里稳定；fork 后前缀事件的 ID 与父 session 一致，便于追溯。
- fork 不重写源文件，compaction 下标不会被破坏。
- replay 完全离线，可用于检查投影和 token 预算。

### 负向

- ID 不是内容哈希：如果手动编辑或重写日志，ID 可能指向不同内容。
- ID 不是全局唯一；跨 session 合并需要额外命名空间。
- fork 重新序列化事件，未来新增的未知字段在复制时可能丢失；当前 schema 已知，可接受。
- 物理行号和事件 ID 在坏行存在时不一致，排查时需要以 `session show` 的序号为准。

## 备选方案

- **给每条事件加 UUID/ULID**：全局唯一，但要改所有构造点，回放不确定，旧日志还要迁移。
- **内容哈希**：能检测篡改，但事件编辑后 ID 变化，fork 追溯会断。
- **物理行号**：实现简单，但坏行或压缩空行会让序号漂移。
- **fork 时插入新的 session_start**：能更新新 session 身份，但会移动后续事件下标，破坏 compaction.upto。
- **fork 直接复制原始行**：保留格式，但坏行会一起复制，且未来字段兼容性更差。

## Revisit 条件

1. 需要跨 session 合并或全局引用事件，引入 `session_id:event_id` 命名空间。
2. 需要内容完整性校验，增加内容哈希字段而不是替换位置 ID。
3. fork 需要保留未知/未来字段，改为复制原始 JSON 值。
4. 需要按时间或语义选择 fork 点，提供时间戳/事件类型选择器。
