+++
id = "M0-005"
title = "Provider trait 与 OpenAI-compatible SSE"
status = "done"
milestone = "M0"
module = "providers"
size = "L"
priority = "P0"
updated = "2026-10-20"
completed = "2026-10-20"
depends_on = ["M0-002"]
docs = ["provider", "architecture"]
+++

## 范围

- 内部统一的 `StreamEvent`：text / reasoning / tool call / usage / done / error。
- 一个真实 provider 的 SSE 适配，优先 OpenAI-compatible。
- 错误分类：可重试、致命、上下文溢出。
- usage 归一化，保留 cached 与 reasoning tokens。

## 验收

- [x] 本地 SSE fixture 能解析出 text、reasoning、tool call、usage、done。
- [x] 429 / 5xx 分类为可重试，并读取 `Retry-After`；实际重试由 loop 在 turn 边界执行。
- [x] 上下文溢出返回独立错误类型，交给 compaction 处理。
- [x] provider 模块不依赖 tools 或 TUI。
- [x] DeepSeek preset：`thinking`、`reasoning_content`、`deepseek-flash` / `deepseek-v4-pro`。

## 记录

实现 `ni-provider`：Provider trait、OpenAI-compatible SSE 客户端、DeepSeek preset、scripted provider，以及本地 TCP + SSE 测试（不访问网络）。录制的 DeepSeek SSE fixture 在 `crates/ni-provider/tests/fixtures/deepseek-tool-call.sse`。当前环境没有 API key，所以没有做真实 DeepSeek 调用；`ni provider test --preset deepseek` 可在配置 key 后验证。
