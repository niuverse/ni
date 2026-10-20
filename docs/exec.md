+++
id = "exec"
title = "ni exec 与最小闭环"
kind = "doc"
status = "living"
order = 32
summary = "headless 任务入口、四个工具、session 事件，以及当前闭环的能力与边界。"
+++

## 用法

```sh
ni auth set deepseek
ni exec "find the failing test and fix it"

# 更便宜/更快的验证
ni exec "read a.txt and summarize it" --no-thinking

# 指定工作区和会话目录
ni exec "fix the build" \
  --cwd /path/to/repo \
  --session-dir /tmp/ni-sessions

# 换模型 / 限制轮数
ni exec "..." --model deepseek-v4-pro --max-steps 20
```

`ni exec` 是 headless 入口：文本流到 stdout，工具调用和 usage 流到 stderr，session 写入 JSONL。

凭证通过 `ni-auth` 解析：环境变量 → OS keyring → 0600 文件。没有凭证时会提示：

```text
no credential for deepseek; run `ni auth set deepseek` or set DEEPSEEK_API_KEY
```

## 闭环结构

```text
session_start
  → user_message
  → provider.stream
  → assistant_message
  → turn_end
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
| `edit` | 精确字符串替换 | 必须唯一匹配；先 `read` |
| `bash` | 在 cwd 执行命令 | 默认 120s 超时；合并输出，只回尾部 20KB |

文件工具会把路径 canonicalize 到工作区内部；`../` 或绝对路径逃逸会被拒绝。`bash` 是逃生舱，不经过这层路径策略。

## 安全边界

当前是最小闭环，不是安全沙箱：

- `write` / `edit` 会修改工作区文件。
- `bash` 可以执行任意命令，也可以访问工作区外路径。
- 没有 approval 提示、没有 OS 级 sandbox、没有 network 开关。
- 默认最大 12 个模型 turn；超过会报错并停止。

因此只在可信仓库、临时工作区或容器里运行。真正的 sandbox / approval 是 M1-002。

## 当前能力

- 真实 DeepSeek `deepseek-flash` 文本 + read 工具调用已验证。
- 本地 mock SSE server 可以完整跑 text 与 tool 两条路径，不消耗 token。
- 脚本化 provider 的 agent 测试覆盖：无工具完成、工具后继续、max steps。
- 每次 turn 写 `assistant_message`、`turn_end`、`tool_result`，usage 含 cached tokens。

## 明确还没有

- `grep` / `glob`（M0-006）。
- bash 进程组 kill、输出全文指针（M0-007）。
- resume / replay / compaction（M0-009、M2）。
- 并行工具、审批、sandbox、TUI。
- 上下文预算；长任务可能撞上下文上限。

## 调试

不烧 token 的路径：

```sh
cargo test -p ni-agent -p ni-tools -p ni-provider
```

本地 mock server：把 `DEEPSEEK_BASE_URL` 指向本地 SSE 服务，用假的 key 执行 `ni exec`。provider 文档里有 SSE 格式和错误分类。
