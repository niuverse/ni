+++
id = "exec"
title = "niucode exec 与最小闭环"
kind = "doc"
status = "living"
order = 32
summary = "headless 任务入口、六个工具、session 事件，以及当前闭环的能力与边界。"
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

# 换模型 / 限制轮数
niucode exec "..." --model deepseek-v4-pro --max-steps 20

# 沙箱三轴
niucode exec "..." --sandbox read-only --network off
niucode exec "..." --sandbox workspace-write --approval ask
niucode exec "..." --sandbox full-access --network on --approval never
```

`niucode exec` 是 headless 入口：文本流到 stdout，工具调用和 usage 流到 stderr，session 写入 JSONL。

凭证通过 `niucode-auth` 解析：环境变量 → OS keyring → 0600 文件。没有凭证时会提示：

```text
no credential for deepseek; run `niucode auth set deepseek` or set DEEPSEEK_API_KEY
```

## 闭环结构

```text
session_start
  → policy
  → user_message
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

`--resume` 会从 `user_message`、`assistant_message`、`tool_result` 重建历史，跳过 `turn_end` / `session_end`，并把最后一个 `compaction` 摘要作为 system 消息。未指定 `--cwd` 时使用 `session_start` 里的 cwd。

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
  - `ask`：每个 write/edit/bash 前在终端询问。
  - `on-failure`：沙箱可用时不问；bwrap 不可用时降级为逐工具询问。
  - `never`：不问；没有 bwrap 且需要隔离时启动直接失败。

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

## 明确还没有

- landlock/seccomp 与 macOS seatbelt（ADR-0006 的后续后端）。
- 可配置 writable roots（cargo 缓存、`/tmp`）。
- compaction / 上下文预算（M2）；长任务可能撞上下文上限。
- 并行工具；当前顺序执行。
- SessionRuntime 的 Command/Event 通道；当前是函数边界。
- fork / subagent。

## 调试

不烧 token 的路径：

```sh
cargo test -p niucode-agent -p niucode-tools -p niucode-provider
```

本地 mock server：把 `DEEPSEEK_BASE_URL` 指向本地 SSE 服务，用假的 key 执行 `niucode exec`。provider 文档里有 SSE 格式和错误分类。
