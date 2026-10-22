+++
id = "M6-021"
title = "TUI invokes the full turn-hook surface"
status = "done"
milestone = "M6"
module = "tui"
size = "M"
priority = "P1"
updated = "2026-10-22"
completed = "2026-10-22"
depends_on = ["M6-020"]
docs = ["adr-0032"]
+++

## 范围

- `TuiOptions` 与 `run_tui_with_hooks(TuiOptions, Box<dyn TurnHooks>)` 是公开入口。
- 共享 `Rc<RefCell<Box<dyn TurnHooks>>>` 只在一个方法调用内借用，不跨 await。
- 启动前调用 `augment_system_prompt`。
- live runner 和无编辑器路径都把 `AgentEvent` 转发给 `on_event`，并在成功 outcome 上调用 `on_turn_end`。
- 编译测试验证公开入口可实例化；全量测试、clippy、dashboard 通过。

## 验收

- [x] TUI prompt hook 调用一次。
- [x] live 路径的事件和 turn-end 调用。
- [x] 无编辑器路径的事件和 turn-end 调用。
- [x] 默认空 hooks 不影响行为。
- [ ] renderer 迁移（下一步，不在本卡）。
