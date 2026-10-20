+++
id = "provider"
title = "Provider 与 DeepSeek 接入"
kind = "doc"
status = "living"
order = 30
summary = "OpenAI-compatible SSE 的归一化、DeepSeek 参数、错误分类和重试边界。"
+++

## 为什么先做 OpenAI-compatible

DeepSeek 和 OpenAI 都提供 Chat Completions 风格接口：`POST /chat/completions`，`stream: true` 返回 SSE，工具调用使用 `tools` / `tool_calls`。先把这一种 wire format 做扎实，比同时维护多套协议更有价值。

Provider 层只做三件事：发请求、解析 SSE、归一化成 `StreamEvent`。它不知道工具如何执行，也不持有 session。

## CLI

```sh
niucode auth set deepseek
niucode provider list

# DeepSeek：默认模型 deepseek-flash，默认开启 thinking
niucode provider test --preset deepseek

# 指定模型 / 关闭 thinking
niucode provider test --preset deepseek --model deepseek-v4-pro --no-thinking

# OpenAI
niucode auth set openai
niucode provider test --preset openai
```

## 环境变量

| 变量 | 默认值 | 说明 |
|---|---|---|
| `DEEPSEEK_API_KEY` | 必填或 `niucode auth` | DeepSeek API key |
| `DEEPSEEK_BASE_URL` | `https://api.deepseek.com` | 不要重复拼 `/chat/completions` |
| `DEEPSEEK_MODEL` | `deepseek-flash` | 也可用 `deepseek-v4-pro` |
| `DEEPSEEK_THINKING` | `enabled` | `enabled` / `disabled` |
| `DEEPSEEK_REASONING_EFFORT` | API 默认（high） | `none` / `low` / `high` / `max` |
| `OPENAI_API_KEY` | 必填或 `niucode auth` | OpenAI key |
| `OPENAI_BASE_URL` | `https://api.openai.com/v1` | 兼容网关可覆盖 |
| `OPENAI_MODEL` | `gpt-4o-mini` | 兼容网关可覆盖 |

模型名会变化。DeepSeek 当前推荐 `deepseek-flash` 与 `deepseek-v4-pro`，以 [官方文档](https://api-docs.deepseek.com/zh-cn/) 为准。

凭证解析顺序是 env → OS keyring → 0600 文件；详见 [凭证与 niucode auth](auth.md)。

## 请求

- `stream: true`
- `stream_options.include_usage: true`
- DeepSeek：`thinking: {"type": "enabled" | "disabled"}`，可选 `reasoning_effort`
- 工具：`tools: [{ "type": "function", "function": { name, description, parameters } }]`

响应是 SSE：

```text
data: {"choices":[{"delta":{"reasoning_content":"..."},"finish_reason":null}]}

data: {"choices":[{"delta":{"content":"..."},"finish_reason":null}]}

data: {"choices":[{"delta":{"tool_calls":[...]}}]}

data: {"choices":[{"delta":{},"finish_reason":"stop"}],"usage":{...}}

data: [DONE]
```

## 归一化事件

- `TextDelta`：`delta.content`
- `ReasoningDelta`：`delta.reasoning_content`
- `ToolCallDelta`：`delta.tool_calls[]`，保留 `index`、`id`、`name`、`arguments` 片段；拼装由 loop 负责
- `Usage`：`prompt_tokens` → input、`completion_tokens` → output、`prompt_tokens_details.cached_tokens` → cached、`completion_tokens_details.reasoning_tokens` → reasoning
- `Done`：`[DONE]` 或流结束，带最后看到的 `finish_reason`

SSE 解析器按行缓冲，支持一个事件跨多个网络 chunk、多行 `data:`、注释/keepalive，以及没有 `[DONE]` 的流。

## DeepSeek 的思考模式

- 默认开启 thinking；`reasoning_content` 与 `content` 同级。
- 请求携带 `tools` 时，历史 assistant 消息里的 `reasoning_content` **必须回传**，否则 API 返回 400。`ChatMessage::assistant` 保留该字段。
- 不携带 `tools` 时，历史 reasoning 不需要回传。
- thinking 模式下 `temperature` 等参数可能被忽略；不要依赖它们做确定性控制。

## 错误与重试

| 情况 | 分类 | 可重试 |
|---|---|---|
| 401 / 403 | `Auth` | 否 |
| 400 / 422 且提示 context length | `ContextOverflow` | 否，交给 compaction |
| 429 / 408 / 425 / 5xx | `Http` | 是，读取 `Retry-After` |
| 402 余额不足 | `Http` | 否 |
| 网络连接失败 | `Network` | 是 |
| SSE JSON 损坏 / 中途断流 | `Decode` | 否（已产生部分输出，不能原地重放） |

**重试边界在 turn 开始前**。一旦工具已经执行或流已经产生输出，不能把整个 turn 重放；应该记录失败并开新 turn。

## 测试

```sh
cargo test -p niucode-provider
```

测试不访问网络：一个本地 TCP server 返回 SSE，验证流式解析；另外覆盖 429 `Retry-After`、context overflow 分类、reasoning/tool call 的序列化。

## 当前限制

- 只支持 Chat Completions；不做 Responses API / Anthropic Messages。
- tool call 参数拼装、执行、审批在 loop/tools，不在 provider。
- 没有读超时和 per-chunk idle timeout；长流依赖取消信号。
- 没有音频/图像输入；v0 只处理文本。

## 学习点

写完这一层，应该能回答：SSE 的 `data:` 和空行如何分帧？一个 tool call 的参数为什么要按片段拼接？`reasoning_content` 为什么在带 tools 的历史里不能丢？429 和“流中途失败”为什么不能用同一种重试？
