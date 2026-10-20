+++
id = "adr-0017"
title = "ADR-0017：Provider seam 与跨 provider model catalog"
kind = "adr"
status = "accepted"
order = 44
summary = "provider/model 是 canonical 选择；catalog 代码内置；凭据惰性解析；model_change 原子记录 provider+model；context budget 由模型窗口派生。"
+++

## 状态

Accepted · 2026-10-20

## 背景

M4 的 `/model` 只能在启动 preset 内切换；跨 provider 需要重启。Pi 的 selection/dispatch 分离说明模型选择应该独立于一次请求；niucode 需要一个小而可测试的 seam，而不是完整 model router。

## 决策

1. **canonical `provider/model`**：选择、CLI、事件统一这个形式；裸 model 只解析到当前 provider。
2. **catalog 在 `niucode-provider` 内静态声明**：provider id、label、API key env、base URL、default model，以及 model 的 context window / max output / tools / reasoning。未知 model 回退 64k + unknown limits。
3. **provider 切换惰性**：只有真正选择/构造 provider 时才解析凭据；catalog 本身不接触 keyring。
4. **原子事件**：`model_change { provider, model }`；旧事件 provider 为空时按当前 config provider 解释。一次选择不会出现 provider/model 分裂。
5. **context budget 从 catalog 派生**：`--context-budget 0`（默认）= `window - 15% reserve`，显式值优先；TUI 切换模型时重新计算。
6. **范围**：M5-001 只做 provider seam；Tool/Loop seam 不混入完成定义。

## 后果

- `/model` 可跨 DeepSeek/OpenAI，无需重启；resume 可恢复选择。
- 静态 catalog 需要随新模型更新；未知模型可用但限制保守。
- 跨 provider 切换是 turn 边界；prompt cache 可能失效。
- 项目级自定义 endpoint、远程 catalog、pricing/vision 留给后续。
