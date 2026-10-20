+++
id = "M5-001"
title = "Provider seam and cross-provider model catalog"
status = "review"
milestone = "M5"
module = "providers"
size = "M"
priority = "P0"
updated = "2026-10-20"
depends_on = ["M4-011"]
docs = ["adr-0017", "provider", "tui"]
+++

## 范围

- 静态 ProviderDescriptor / ModelDescriptor catalog。
- canonical `provider/model`；跨 provider `/model`。
- 惰性凭据解析与运行时 ProviderFactory。
- `model_change { provider, model }` 原子事件。
- context budget 从模型 context window 派生；`--context-budget 0` = auto。
- TUI 下拉、`/status`、`/clear`/`/reset` 的 provider 状态。

## 验收

- [x] DeepSeek/OpenAI catalog 与 canonical 选择解析。
- [x] TUI 从 DeepSeek 切到 OpenAI 时重建 provider，不发起网络请求也可验证。
- [x] model_change 记录 provider；/status 显示 provider/model。
- [x] 默认预算由模型窗口派生；显式 --context-budget 覆盖。
- [ ] `/model` 下拉应按凭据可用性禁用 provider；当前是选择时检查并报错。
- [ ] `exec` 的跨 provider `--model provider/model` 尚未完成同等 wiring。
- [ ] SessionStart 尚未写 provider。

## 记录

- `niucode-provider` 新增 catalog、ModelSelection、derived_context_budget。
- CLI 新增 ProviderFactory：惰性 AuthStore、按 provider 构造 OpenAiProvider、记录 wrapper 兼容。
- TUI 的 run 接受 Arc<dyn Provider> + factory；切换失败不修改 state。
- `context_budget_auto` 只在 CLI 显式 0（默认）时开启；TUI 每 turn 重算。
- 已知边界见未勾选项；M5-001 暂为 review，不提前标 done。
