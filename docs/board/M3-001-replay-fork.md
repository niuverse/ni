+++
id = "M3-001"
title = "Replay 与 fork"
status = "done"
milestone = "M3"
module = "session"
size = "M"
priority = "P2"
updated = "2026-10-20"
completed = "2026-10-20"
depends_on = ["M2-001"]
docs = ["adr-0002", "adr-0010", "exec"]
+++

## 范围

- `niucode replay <session>`：按事件重建状态并校验投影。
- `niucode fork <session> --at <event-id>`：复制前缀，生成新 session。
- 事件 id 稳定，fork 后历史可追溯。

## 验收

- [x] replay 不调用模型，输出与原始 turn 的投影一致。
- [x] fork 后原 session 不被修改。
- [x] 坏尾行 fork 时被截断并提示。
- [x] 事件 id 冲突检测。

## 记录

事件 ID 定义为**解码事件的 1-based 序号**，显示为 `e000012`，解析接受 `e12` / `E12` / `12`。ID 不写进事件内容；append-only 保证序号稳定，`load` 跳过坏行，坏行不制造 ID。

- `niucode replay <session>` 对每个 `user_message` 投影到“下一个 assistant 消息之前”，包含请求前追加的 compaction / instructions；输出 turn、消息数、估算 token 和投影 digest。没有构造 provider。
- `niucode fork <session> --at <id>` 校验 ID、复制 `events[0..=index]`、在末尾追加 `Fork { parent, parent_event, ts }`。不插入前缀事件，因此 `compaction.upto` 保持有效；复制的 `session_start` 保留父 id，新身份由文件名和 fork 事件表达。
- 目标文件用 `SessionLog::create` 的 `create_new` 创建，不覆盖已有文件；源文件只读。fork 默认写到源 session 同目录，可用 `--session-dir` 覆盖。
- 坏尾行由 `load` 报告 warning；fork 只复制有效前缀，因此坏尾行被截断并提示。
- ID 唯一性由序号构造保证；命令检测非法/越界 ID 和目标文件创建冲突。

实现位于 `crates/niucode-cli/src/session_ops.rs`。测试覆盖：replay 在 user 后追加 compaction 的投影、fork 前缀与溯源事件、原文件字节不变、坏行跳过并 warning、compaction cutoff 在 fork 后仍有效、非法/越界 ID 拒绝。CLI 上跑过真实 session 的 replay 与 fork。

决策见 ADR-0010。当前 ID 不是内容哈希，也不是全局唯一；跨 session 合并时需要 `session_id:event_id` 命名空间。
