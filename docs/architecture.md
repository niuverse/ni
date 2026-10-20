+++
id = "architecture"
title = "架构"
kind = "doc"
status = "living"
order = 10
summary = "ni 的边界、数据流、事件生命周期与明确不做的事。"
+++

## 一句话

ni 是一个围绕 **append-only 事件日志** 构建的最小 coding agent harness：一个 turn 引擎、一层薄 provider 适配、六个工具、三条安全轴。其余都是外围。

## 设计约束

1. **日志是唯一真相。** 内存里的对话是日志的投影。任何不落盘的状态，进程退出即不存在。
2. **上下文是产品。** 模型看到什么、按什么顺序看到、何时压缩，决定了 agent 的上限。
3. **工具少而锋利。** 每多一个工具，就多一份 schema、一类失败模式和一段 prompt。
4. **provider 层要薄。** 只归一化够用的部分，不试图抹平所有 wire format 怪癖。
5. **安全边界要诚实。** 进程内路径校验不是沙箱；bash 能绕过的东西，不能假装它不能。

## 数据流

```text
user input
   │
   ▼
 CLI ──► loop ──► context.project(log, budget) ──► provider.stream
            │                                          │
            │                                     StreamEvent
            │                                          ▼
            │                                   assistant message
            │                                          │
            ├────────────── tool calls ◄───────────────┘
            │                   │
            │                   ├──► sandbox.guard
            │                   └──► tools.dispatch (read-only 并发，写串行)
            │
            └──► session.append(every event, flush)
```

一个 turn 的边界是 `user_message` 到 `turn_end`。loop 只做四件事：投影、请求、派发、追加。它不解析 SSE，不实现工具，不渲染 UI。

## 事件生命周期

| 事件 | 何时写入 | 关键字段 |
|---|---|---|
| `session_start` | 创建会话 | id、cwd、model |
| `user_message` | 用户输入 | text |
| `assistant_message` | 一次模型响应结束 | text、reasoning、tool_calls |
| `tool_result` | 每个工具执行完 | call_id、ok、output |
| `turn_end` | 一个 turn 结束 | usage（含 cached / reasoning） |
| `compaction` | 上下文压缩 | summary、upto |
| `session_end` | 正常退出 | ts |

`compaction.upto` 表示被摘要替换的事件数量。resume 时先加载全量事件，再按最后一个 compaction 截断投影；原始事件永远保留。

## 模块边界

| 层 | 模块 | 职责 |
|---|---|---|
| Interface | CLI / TUI | 参数、生命周期、渲染；不持有对话状态 |
| Engine | Loop / Context | 编排、投影、压缩；不碰 wire format |
| Adapters | Providers / Tools / Sandbox / Auth | 外部世界与凭证的适配；不依赖 UI |
| State | Session | 追加、加载、容错；不解释业务 |
| Engineering | Dashboard | 架构阅读面（默认 Architecture）；不参与运行时 |

依赖方向只能向下：Interface → Engine → Adapters / State。`ni-core` 不依赖 `ni-cli`，TUI 不 import loop 内部。

## 工具面

第一版只有六个：`read`、`write`、`edit`、`grep`、`glob`、`bash`。

- `read` 带行号，单次上限约 2000 行 / 50KB，截断时给出续读方式。
- `edit` 用精确字符串替换，要求唯一匹配，返回小 diff；行号会漂移，不用行号。
- `bash` 只回尾部约 20KB，完整输出落临时文件；超时杀进程组，不留孤儿。
- 工具错误是 prompt 的一部分：写清楚"为什么失败、下一步怎么做"。

`write` / `edit` 前必须读过目标文件。这是策略层强制，不是提示词建议。

## 上下文与压缩

- `project(log, budget)` 是全量日志到模型上下文的有损投影。
- system + tools + 历史前缀保持稳定，易变内容放末尾，以命中 provider 的 prompt cache。
- 到预算 75% 触发 compaction，保留最近 N 轮原文和原始请求，生成结构化 handoff：目标、已完成、改动文件、当前状态、下一步、约束。
- 压缩结果落成事件，不修改历史。

## 安全

三条正交的轴，不要揉成一个开关：

- **sandbox**：read-only / workspace-write / full-access。
- **approval**：ask / on-failure / never。
- **network**：on / off。

沙箱用平台原生能力（Linux landlock + seccomp，macOS seatbelt）。没有真沙箱的平台，明确降级为"变更型 bash 强制审批"，并告诉用户原因。

## 明确不做

- 不做通用 agent framework，不为未知用例抽象。
- v0 不做 MCP、插件系统、多 provider 全家桶、subagent、Web UI。
- 不做浏览器里可编辑的看板；状态变更走仓库。
- 不在 loop 里做 prompt 字符串拼接；prompt 是独立、可版本化的输入。

## 开放问题

1. `assistant_message` 是否要保留 reasoning 原文，还是只保留摘要？不同 provider 的回传要求不同。
2. compaction 的 handoff 由模型生成还是模板生成？前者灵活，后者稳定。
3. 工具并行度：只读并发之外，是否允许不同文件的写并发？
4. 事件 schema 的演进策略：加字段兼容，还是显式版本迁移？
