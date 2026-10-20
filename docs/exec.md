+++
id = "exec"
title = "niucode exec 与最小闭环"
kind = "doc"
status = "living"
order = 32
summary = "headless 任务入口、六个基础工具 + subagent + sidecar external tools、JSONL 协议、session 事件、上下文压缩、分层 AGENTS.md 与缓存指标。"
+++

## 用法

```sh
niucode auth set deepseek
niucode exec "find the failing test and fix it"

# 更便宜/更快的验证
niucode exec "read a.txt and summarize it" --no-thinking

# 指定工作区和会话目录
niucode exec "fix the build" \
  --cwd /path/to/repo \
  --session-dir /tmp/niucode-sessions

# 换模型 / 限制轮数 / 上下文预算
niucode exec "..." --model deepseek-v4-pro --max-steps 0
# sessions default to ~/.niucode/projects/<key>/sessions/
niucode exec "..." --context-budget 128000 --keep-recent-turns 4

# 机器可读的 JSONL 事件流
niucode exec "..." --json > events.jsonl

# 沙箱三轴
niucode exec "..." --sandbox read-only --network off
niucode exec "..." --sandbox workspace-write --approval ask
niucode exec "..." --sandbox full-access --network on --approval never
```

`niucode exec` 是 headless 入口：默认文本流到 stdout，工具调用和 usage 流到 stderr，session 写入 JSONL；加 `--json` 时 stdout 改为 JSONL 事件流。

凭证通过 `niucode-auth` 解析：环境变量 → OS keyring → 0600 文件。没有凭证时会提示：

```text
no credential for deepseek; run `niucode auth set deepseek` or set DEEPSEEK_API_KEY
```

## 闭环结构

```text
session_start
  → policy
  → user_message
  → compaction (?)
  → provider.stream
  → assistant_message
  → turn_end
  → approval (?)
  → tool_result (0..n)
  → provider.stream (下一 turn)
  → ...
  → session_end
```

一个 turn 的事件顺序固定：assistant 消息先落盘，再落工具执行结果，然后进入下一轮。工具结果不会丢；resume 可以从同一份日志重建。

## 工具

| 工具 | 行为 | 约束 |
|---|---|---|
| `read` | 带行号读取文本 | 最多 2000 行 / 50KB；支持 offset/limit |
| `write` | 新建或完整覆盖 | 已存在的文件必须先 `read` |
| `edit` | 精确字符串替换 | 必须唯一匹配；先 `read`；返回 unified diff |
| `grep` | 正则搜索文本 | 尊重 `.gitignore`；结果上限 200 |
| `glob` | glob 列文件 | 尊重 `.gitignore`；结果上限 200 |
| `bash` | 在 cwd 执行命令 | 独立进程组；默认 120s 超时；尾部 20KB + 临时输出路径 |
| `subagent` | 并行只读探索 | 1–4 个任务；仅 read/grep/glob；独立日志；返回摘要 + 路径 |

文件工具会把路径 canonicalize 到工作区内部；`../` 或绝对路径逃逸会被拒绝。`bash` 是逃生舱，不经过这层路径策略。

## Resume 与录制回放

```sh
# 从已有 session 继续
niucode exec "continue from here" --resume /path/to/session.jsonl

# 录制归一化 provider turn
niucode exec "record this" --record /tmp/fixture.json

# 完全离线重放，不需要 API key
niucode exec "replay this" --replay /tmp/fixture.json
```

`--resume` 会从 `user_message`、`assistant_message`、`tool_result` 重建历史，跳过 `turn_end` / `session_end`；如果存在 `compaction`，按最后一个事件的 `upto` 在 cutoff 处插入 handoff system 消息。未指定 `--cwd` 时使用 `session_start` 里的 cwd；system prompt 会按当前 cwd 重新发现 AGENTS.md。

Fixture 是归一化事件，不是原始 SSE：

```json
{
  "schema_version": 1,
  "turns": [
    [
      {"type": "tool_call_delta", "index": 0, "id": "call_1",
       "name": "read", "arguments": "{\"path\":\"a.txt\"}"},
      {"type": "done", "finish_reason": "tool_calls"}
    ],
    [
      {"type": "text_delta", "text": "read it"},
      {"type": "done", "finish_reason": "stop"}
    ]
  ]
}
```

离线回归：

```sh
cargo test -p niucode-agent --test golden
cargo test -p niucode-provider fixture::tests
```

## Replay 与 fork

```sh
# 不调用模型，按事件重建每个 user turn 的投影
niucode replay /path/to/session.jsonl

# 复制到指定事件为止的前缀，生成新 session
niucode fork /path/to/session.jsonl --at e000012

# 指定新 session 目录（默认与源文件同目录）
niucode fork /path/to/session.jsonl --at e000012 --session-dir /tmp/forks
```

- 事件 ID 是解码事件的 1-based 序号，显示为 `e000012`，不是物理行号；坏行不制造 ID。
- `replay` 不构造 provider。每个 `user_message` 投影到下一个 assistant 消息之前，包含请求前追加的 compaction / instructions，输出消息数、估算 token 和 digest。
- `fork` 只读源文件，复制 `events[0..=index]`，再追加 `fork` 溯源事件；源文件不变。
- fork 不向前缀插入事件，所以 `compaction.upto` 仍然有效。
- `load` 的坏行 warning 会打印；fork 只复制有效前缀。非法、越界或指向不存在事件的 ID 会直接失败。
- 目标文件用 `create_new` 创建，不覆盖已有 session。

决策见 ADR-0010。

## 上下文与压缩

每个 provider 请求前按估算 token 检查预算：默认 64k，到达 75% 触发 compaction。默认保留最近 3 个 user turn 原文；更早的事件生成确定性 handoff，包含目标、上一次请求、已完成工具动作、成功写入的文件、最近助手状态、近期失败、约束和下一步。

```sh
niucode exec "long task" --context-budget 128000 --keep-recent-turns 4
niucode tui  --context-budget 128000 --keep-recent-turns 4
```

- `compaction.upto` 是绝对事件下标；最后一个 compaction 生效，原始事件永远保留。
- 压缩成功才追加 `compaction` 事件，并记录 `before_tokens` / `after_tokens` 估算。
- 压缩不收缩、没有安全 cutoff 或写入失败时，保留原上下文并输出 warning。
- 估算是启发式（ASCII 约 4 字符/token，非 ASCII 1 字符/token）；真实成本仍看 `turn_end.usage`。
- TUI / exec 在压缩后显示 `compacted: N events · ~before → ~after tokens`；session 可以 resume 回放同一投影。
- `niucode session show <path>` 会打印 compaction 的 `upto` 和 before/after 估算。
- 单个 turn 很长且没有多余 user turn 时，cutoff 回退到该 turn 内安全的 assistant 边界，避免切开 tool call 和 tool result。

实现决策见 ADR-0008。

## Subagent

主模型可以调用 `subagent` 并行跑探索或 Worker 任务；任务不共享上下文，也不能递归：

```json
{
  "tasks": [
    {"prompt": "find the failing test and report file:line", "profile": "explore"},
    {"prompt": "implement the smallest fix in src/auth", "profile": "worker", "write_scope": ["src/auth"]}
  ]
}
```

- `profile: explore`（默认）：`read/grep/glob` + 只读 external tools；sandbox read-only + network off。
- `profile: worker`：`read/write/edit/grep/glob/bash` + 只读 external tools；继承父 sandbox，写操作共享全局 write lock。
- `write_scope` 只约束 write/edit 路径；bash 不受路径 scope 约束，但会被同一个写锁串行化。
- 父 session 记录 `subagent_spawn`（含 profile）/ `subagent_result`；每个任务一个独立 session，默认在 `<parent-stem>.subagents/`。
- 默认 `max_steps=6`（上限 12）、`timeout_secs=120`（上限 600）；context budget 为 `min(parent, 16k)`。
- 父模型只收到一条 tool_result：每个任务的摘要 + 子日志路径；完整 transcript 不进入父上下文。
- 超时/provider 错误/max steps 都返回 `ok=false`，父 agent 继续。
- mutating external tools 不进入 explore/worker；`full-access` 也不默认下放。
- `profile: inherit` 需要显式 `--allow-subagent-inheritance`；它复制父工具面，但一定剥离 `subagent`，不能递归。决策见 ADR-0011/0014。

## External tools

`niucode exec` / `niucode tui` 会扫描 `<workspace>/.niucode/extensions/*.toml`，为每个 manifest 启动一个本地 sidecar，把它在 `initialize` 中声明的工具并入 agent 的 tool specs。

```toml
name = "demo"
command = "python3"
args = ["sidecar.py"]
timeout_secs = 30
```

sidecar 是 line-delimited JSON-RPC：

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocol":1,"client":"niucode","cwd":"/repo"}}
{"jsonrpc":"2.0","id":1,"result":{"name":"demo","version":"0.1.0","tools":[{"name":"demo_echo","description":"Echo text","input_schema":{"type":"object","properties":{"text":{"type":"string"}}}}]}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"demo_echo","arguments":{"text":"hi"}}}
{"jsonrpc":"2.0","id":2,"result":{"ok":true,"output":"echo: hi"}}
```

- 工具名不能与内置工具或其他插件重名；重名会拒绝挂载。
- sidecar 启动时 `env_clear`，只保留解释器启动所需的 PATH/HOME 等；不会继承 API key。
- v1 是**可信本地 sidecar**：每插件串行调用，没有 worker pool、bwrap 网络/能力隔离、subagent 共享 host；这些在 M4-007。
- 父 session 记录 `extension_mount` / `extension_unmount` / `extension_error`；工具调用本身仍走 `tool_result`。
- 没有 manifest 时不产生任何进程。

## JSONL 协议

`niucode exec --json` 把事件流写到 stdout，一行一个 JSON 对象；stderr 继续放人类诊断。第一行是带版本号的 `start`：

```json
{"type":"start","protocol":1,"session":"/path/session.jsonl","model":"deepseek-flash","provider":"openai","cwd":"/repo","sandbox":"workspace-write","network":"off","approval":"on-failure","backend":"bwrap"}
{"type":"text_delta","text":"I will inspect the test."}
{"type":"tool_call","name":"read","input":{"path":"a.txt"}}
{"type":"tool_result","name":"read","ok":true,"summary":"     1  hello"}
{"type":"turn_end","usage":{"input_tokens":120,"output_tokens":16,"cached_tokens":80,"reasoning_tokens":0,"cache_hit_percent":66}}
{"type":"done","turns":2,"usage":{"input_tokens":240,"output_tokens":32,"cached_tokens":180,"reasoning_tokens":0,"cache_hit_percent":75},"session":"/path/session.jsonl"}
```

事件类型：`start`、`text_delta`、`reasoning_delta`、`tool_call`、`tool_result`、`turn_end`、`compaction`、`warning`、`done`、`cancelled`、`error`。

- `tool_result` 只包含 `name` / `ok` / `summary`；完整 output、reasoning 和事件历史在 `session` 指向的 JSONL 里。
- `--json` 不交互：`--approval ask` 会被拒绝；没有 bwrap 导致 `on-failure` 降级为 ask 时也会拒绝。TUI 仍是唯一交互审批入口。
- 成功/取消退出码为 0；provider/模型错误退出码非零，stdout 仍以 `error` 事件结束。
- 协议只做加法兼容；已有字段含义不变。决策见 ADR-0012。

## AGENTS.md 与缓存指标

每次 run 从 cwd 向上发现 `AGENTS.md`，按距离由近到远合并，近者优先，追加在稳定的 base system prompt 之后。修改文件后下一次用户 turn 生效，无需重启。

- 单文件上限 32 KiB，总上限 64 KiB；超出会截断并 warning。
- 加载来源与合并文本 digest 写入 `instructions` 事件；`session show` 会显示文件数和 digest。
- 缓存命中率由 `turn_end.usage` 的 cached / input 派生。TUI 与 exec 在每个 turn 和 `done` 行显示 `cache N%`；`session show` 显示每 turn 的 `in/out/cached/cache`。
- 不写单独的 metrics JSONL；运行时 Runs / metrics 视图属于未来的 runtime viewer，不进入离线项目看板。决策见 ADR-0009。

## 安全边界

默认是 `workspace-write + network=off + approval=on-failure`：

- **sandbox**
  - `read-only`：文件工具拒绝 write/edit；bash 用只读根。
  - `workspace-write`：只能写 cwd；`/` 以只读方式 bind。
  - `full-access`：不套 bwrap，必须显式选择。
- **network**
  - `off`：bwrap `--unshare-all`，子进程无法建立连接。
  - `on`：`--share-net`；只在需要时开启。
- **approval**
  - `ask`：每个 write/edit/bash 前在终端询问；write/edit 展示真实文件的完整 unified diff。
  - `on-failure`：沙箱可用时不问；bwrap 不可用时降级为逐工具询问。
  - `never`：不问；没有 bwrap 且需要隔离时启动直接失败。

审批渲染分两层：`niucode-tools` 生成结构化 `ChangePreview`，headless 输出纯文本 unified diff，TUI 输出可滚动、可按文件跳转的 pager。预览本身不写事件；审批通过后实际执行的 `tool_result` 才落盘，执行前会重新读取并校验文件。

Linux 第一后端是 bwrap，需要安装 `bubblewrap`。它仍是 namespace 隔离，不是 VM；工作区外文件默认可读。`workspace-write` 目前只允许写 cwd，cargo registry / `~/.cargo` 等缓存目录会写入失败。

每次运行的策略写 `policy` 事件，人工审批写 `approval` 事件；拒绝的 tool_result 也落盘。

## 当前能力

- 真实 DeepSeek `deepseek-flash` 文本 + read 工具调用已验证。
- bwrap workspace-write 阻断工作区外写入；network off 阻断连接；read-only 阻断文件工具变更。
- 逐工具 approval 的允许/拒绝路径都有测试。
- 本地 mock SSE server 可以完整跑 text 与 tool 两条路径，不消耗 token。
- 脚本化 provider 的 agent 测试覆盖：无工具完成、工具后继续、max steps。
- 每次 turn 写 `assistant_message`、`turn_end`、`tool_result`，usage 含 cached tokens。
- scripted provider 测试覆盖 read → edit → bash 修掉一个失败 fixture。
- Ctrl-C 会 drop provider stream 和工具 future；bash 的进程组会被清理。
- 审批前可查看完整 diff；大 diff 在 TUI 中分页，超出阈值的工具输出折叠为摘要。
- 上下文到 75% 预算触发 compaction；确定性 handoff 落成事件，resume 回放一致，失败保留原上下文。
- 分层 `AGENTS.md` 按 cwd 向上发现，近者优先；`instructions` 事件记录来源与 digest。
- 每个 turn 的缓存命中率可见，`session show` 可查历史 usage。
- `replay` 可离线重建每个 user turn 的投影；`fork` 可复制前缀并保留 compaction 下标。
- `exec --json` 提供带 `protocol: 1` 的稳定 JSONL 事件流，适合脚本和编辑器集成。
- `subagent` 并行只读探索，父 session 记录 spawn/result，父上下文只增加一条摘要。
- external tools 通过 `.niucode/extensions/*.toml` 挂载，走同一 tool_result 事件。

## 明确还没有

- landlock/seccomp 与 macOS seatbelt（ADR-0006 的后续后端）。
- 可配置 writable roots（cargo 缓存、`/tmp`）。
- 与模型匹配的精确 tokenizer；当前预算使用字符估算。
- 模型生成的语义摘要；当前 handoff 是确定性模板。
- 运行时 Runs / metrics viewer；当前缓存指标在 session 事件与 CLI / TUI 中。
- 并行工具；当前顺序执行。
- SessionRuntime 的 Command/Event 通道；当前是函数边界。
- 运行时 subagent 的可视化面板；当前只看 tool 状态与摘要。

## 调试

不烧 token 的路径：

```sh
cargo test -p niucode-agent -p niucode-tools -p niucode-provider
```

本地 mock server：把 `DEEPSEEK_BASE_URL` 指向本地 SSE 服务，用假的 key 执行 `niucode exec`。provider 文档里有 SSE 格式和错误分类。
