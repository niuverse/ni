+++
id = "architecture"
title = "架构"
kind = "doc"
status = "living"
order = 10
summary = "niucode 的边界、数据流、事件生命周期与明确不做的事。"
+++

## 一句话

niucode 是一个围绕 **append-only 事件日志** 构建的最小 coding agent harness：一个 turn 引擎、一层薄 provider 适配、工具面、三条安全轴。当前最小闭环已有六个基础工具和一个只读 subagent 编排工具，Linux bash 默认走 bwrap 沙箱。

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
| `instructions` | 每次 run 发现 AGENTS.md | sources、digest |
| `fork` | fork 出新 session | parent、parent_event |
| `subagent_spawn` | 父 session 启动只读子任务 | id、call_id、task、session、预算 |
| `subagent_result` | 子任务结束 | id、ok、summary、session、turns、usage |
| `plan_mode` | plan mode 开关 | active |
| `goal_set` / `goal_cleared` | 会话目标 | text / 清除 |
| `goal_verdict` | `/goal run` evaluator 结论 | status、reason |
| `model_change` | 运行时模型选择 | model |
| `permission_change` | 运行时权限 profile | profile、sandbox、approval、network |
| `extension_mount` / `extension_unmount` | sidecar 挂载/卸载 | name、version、tools |
| `extension_error` | sidecar 挂载/调用失败 | name、message |
| `compaction` | 上下文压缩 | summary、upto、before/after tokens |
| `session_end` | 正常退出 | ts |

`compaction.upto` 是事件日志的绝对下标：摘要替换 `events[..upto]`。compaction 事件本身追加在日志末尾，但投影时在 `upto` 处插入摘要。resume 只使用最后一个 compaction；原始事件永远保留。

事件 ID 是解码事件的 1-based 序号（`e000012`），不写进事件内容；append-only 保证稳定，坏行不制造 ID。`replay` 按 user turn 重建投影，不调用模型；`fork` 复制前缀并在末尾追加 `fork` 事件，前缀下标和 `compaction.upto` 不变。见 ADR-0010。

## 模块边界

| 层 | 模块 | 职责 |
|---|---|---|
| Interface | CLI / TUI | 参数、生命周期、渲染；不持有对话状态 |
| Engine | Loop / Context | 编排、投影、压缩；不碰 wire format |
| Adapters | Providers / Tools / Sandbox / Auth | 外部世界与凭证的适配；不依赖 UI |
| State | Session | 追加、加载、容错；不解释业务 |
| Engineering | Dashboard | 架构阅读面（默认 Architecture）；不参与运行时 |

依赖方向只能向下：Interface → Engine → Adapters / State。`niucode-core` 不依赖 `niucode-cli`，TUI 不 import loop 内部。

## 工具面

六个基础工具：`read`、`write`、`edit`、`grep`、`glob`、`bash`；M3 增加一个只读 `subagent` 编排工具。

- `read` 带行号，单次上限约 2000 行 / 50KB，截断时给出续读方式。
- `edit` 用精确字符串替换，要求唯一匹配，返回小 diff；行号会漂移，不用行号。
- `bash` 只回尾部约 20KB，完整输出落临时文件；超时杀进程组，不留孤儿。
- 工具错误是 prompt 的一部分：写清楚"为什么失败、下一步怎么做"。

`write` / `edit` 前必须读过目标文件。这是策略层强制，不是提示词建议。

审批前由 `Toolbox::preview` 读取真实文件，生成结构化 unified diff（文件、hunk、行号、增删行）。loop 只转发 `ChangePreview`，TUI 与 headless 各自渲染；UI 不复制路径策略，预览也不修改工作区。决策见 ADR-0007。

## Subagent

`subagent` 是第七个工具，也是唯一的编排工具：接收 1–4 个任务，并行运行，每个任务一个独立 session 日志。

- `profile: explore`（默认）：`read` / `grep` / `glob` + 只读 external tools；sandbox read-only + network off；没有 write/edit/bash，也没有 `subagent`，不能递归。
- `profile: worker`：`read` / `write` / `edit` / `grep` / `glob` / `bash` + 只读 external tools；继承父 sandbox；`write_scope` 约束 write/edit 路径。
- W1：父和所有 Worker 共享一个全局 write lock；write/edit/bash 串行。bash 不受 path scope 约束。
- `profile: inherit` 需要显式 `--allow-subagent-inheritance`；复制父工具面，但剥离 `subagent`，不能递归。
- mutating external tools 和 full-access 不默认下放给子 agent。
- 父 session 记录 `subagent_spawn`（含 profile）/ `subagent_result`，包含子 session 路径；子 `session_start.id` 等于 spawn id。
- 父模型只收到一条 tool_result：每个任务的摘要 + 日志路径；完整 transcript 留在子日志里。
- 每个任务有独立 `max_steps`、`timeout_secs` 和 context budget；超时/失败返回 `ok=false`，父 agent 继续。决策见 ADR-0011/0014。

## Plan mode 与 Goal

`/plan` 是运行时 mode，不是 profile：进入后 tool allowlist 收窄到 `read/grep/glob`，system prompt 注入只读计划指令；退出恢复原 profile。`/goal <condition>` 在 TUI 中设置目标并立即启动 goal run（最多 6 轮）；`/goal run` 重新启动已有目标，`/goal stop` / `/goal clear` 停止。每轮结束用独立无工具 evaluator 写 `goal_verdict`；goal 不改变权限。headless `exec --goal` 是 M4-008。决策见 ADR-0014。

## External tools

`niucode-extension` 是一个 session 级 sidecar host。它在 `<workspace>/.niucode/extensions/*.toml` 发现 manifest，spawn 本地进程，通过 line-delimited JSON-RPC 完成 `initialize` / `tools/call` / `shutdown`。工具在 `initialize` 时注册，合并进 agent 的 tool specs；调用按插件串行（M4-002 的并发模型是保守的）。sidecar 启动时 `env_clear`，不继承 API key；超时/崩溃返回 tool error。父 session 追加 `extension_mount` / `extension_unmount` / `extension_error`。没有 manifest 时不产生任何进程。capability sandbox、worker pool、subagent 共享 host 是 M4-007。决策见 ADR-0013。

## JSONL 协议

`niucode exec --json` 在 stdout 输出一行一个 JSON 事件，第一行 `start` 带 `protocol: 1`。事件类型与 `AgentEvent` 同构，另有 `start` / `done` / `cancelled` / `error`。`tool_result` 只含 summary，完整 output 在 session 日志。stderr 保留人类诊断；`--approval ask` 在 JSON 模式下被拒绝。决策见 ADR-0012。

## 上下文与压缩

- `project_history(events)` 是全量日志到模型上下文的有损投影；`compaction` 在 cutoff 处插入一条 system handoff。
- system + tools + 历史前缀保持稳定，易变内容放末尾，以命中 provider 的 prompt cache。
- 每次 run 从 cwd 向上发现 `AGENTS.md`，近者优先，追加在稳定 base system prompt 之后；修改后下一次用户 turn 生效。加载结果记录为 `instructions` 事件（sources + digest），用于定位 prompt cache 前缀变化。
- 到估算预算 75% 触发 compaction：默认保留最近 3 个 user turn 原文，其余生成确定性 handoff：目标、上一次请求、已完成、改动文件、最近助手状态、近期失败、约束、下一步。
- 单个 turn 很长时，回退到该 turn 内安全的 assistant 边界，保留尾部；tool call 和 tool result 不会被切开。
- 先构造候选投影并确认 token 估算下降，才把 `compaction` 事件落盘；失败保留原上下文并提示。
- 压缩结果落成事件，不修改历史。token 估算见 ADR-0008。
- 缓存命中率由 `turn_end.usage` 的 cached / input 派生，TUI、exec 与 `session show` 直接显示；不写第二份 metrics 文件。见 ADR-0009。

## 安全

三条正交的轴，不要揉成一个开关：

- **sandbox**：read-only / workspace-write / full-access。
- **approval**：ask / on-failure / never。
- **network**：on / off。

Linux 第一后端用 bwrap namespace：只读根 + workspace bind，network off 用独立 net namespace。没有 bwrap 时，`never` 启动失败，`on-failure` / `ask` 降级为逐工具审批并提示网络策略未执行。landlock/seccomp 与 macOS seatbelt 是后续后端，见 ADR-0006。

路径策略与 shell 隔离分开：文件工具在进程内 canonicalize；bash 的外部隔离由 bwrap 提供。两者都不是 VM，工作区外文件默认可读。

## 明确不做

- 不做通用 agent framework，不为未知用例抽象。
- 不做通用 everything-is-plugin、MCP 全家桶、多 provider 全家桶、通用多 agent 框架；M4 只做 capability 级 external tools/commands（ADR-0013），loop/context/policy/auth 仍在 kernel。
- 不在离线项目看板里加运行时 Runs / metrics 视图；那属于未来的 runtime viewer（ADR-0009）。
- 不做浏览器里可编辑的看板；状态变更走仓库。
- 不在 loop 里做 prompt 字符串拼接；prompt 是独立、可版本化的输入。

## 开放问题

1. `assistant_message` 是否要保留 reasoning 原文，还是只保留摘要？不同 provider 的回传要求不同。
2. 确定性 handoff 在什么任务上开始丢意图？需要模型生成 handoff 时，如何限制额外调用成本？
3. 工具并行度：只读并发之外，是否允许不同文件的写并发？
4. 事件 schema 的演进策略：加字段兼容，还是显式版本迁移？
