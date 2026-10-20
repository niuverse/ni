+++
id = "M0-005"
title = "Provider trait 与流式适配"
status = "ready"
milestone = "M0"
module = "providers"
size = "L"
priority = "P0"
updated = "2026-10-20"
depends_on = ["M0-002"]
docs = ["architecture"]
+++

## 范围

- 内部统一的 `StreamEvent`：text / reasoning / tool call / usage / done / error。
- 一个真实 provider 的 SSE 适配，优先 OpenAI-compatible。
- 错误分类：可重试、致命、上下文溢出。
- usage 归一化，保留 cached 与 reasoning tokens。

## 验收

- 录制的 SSE fixture 能解析出完整 turn。
- 429 / 5xx 在 turn 边界重试，工具执行后不重放。
- 上下文溢出返回可识别错误，交给 compaction 处理。
- provider 模块不依赖 tools 或 TUI。

## 记录

先做一个 provider。第二个 provider 等协议稳定后再加，避免过早抽象。
