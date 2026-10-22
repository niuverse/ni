+++
id = "adr-0032"
title = "ADR-0032：向嵌入方暴露可复用前端与 turn hooks"
kind = "adr"
status = "accepted"
order = 59
summary = "把 TUI 与 runtime 暴露为库，并提供通用 hook：prompt 追加、turn 结束、事件观察；不引入产品类型，不新增第二状态。"
+++

## 状态

Accepted · 2026-10-22。来自 niuse 的嵌入需求；上游讨论见 [#1](https://github.com/niuverse/niucode/issues/1)。

第一步已落地：`niucode-cli` 增加库目标，导出 `run()`、`TurnHooks` 与 `NoHooks`，并有一个链接测试。

第二步在 exec 路径落地：`run_exec_with_hooks(ExecOptions, &mut dyn TurnHooks)` 会在构造 `AgentConfig` 后调用 `augment_system_prompt`，在运行时事件上调用 `on_event`，并在首个 turn 结束后调用 `on_turn_end`；集成测试用 replay fixture 断言三者都发生。

TUI 路径已有 `TuiOptions` 与 `run_tui_with_hooks`，启动前调用 `augment_system_prompt`；TUI 循环与 live runner 通过共享的 `Rc<RefCell<Box<dyn TurnHooks>>>` 短借用，事件观察和 turn-end 均已接线。PTY + replay 的 example 探针确认三者都实际触发；renderer 迁移尚未开始。

## 背景

niuse 需要复用 niucode 的 TUI、审批、diff 与 transcript，但 `niucode-cli` 目前只有 bin、没有 lib。结果是 niuse 自己驱动 turn、自己维护 renderer：同一套前端要演进两次，而且 niuse 无法复用终端验收。

niucode 的边界不能因此改变：内核仍然是最小 harness，不认识身份、记忆或任何伴侣概念。需要暴露的是**通用的前端与生命周期 seam**，不是产品能力。

## 决策

1. `niucode-cli` 增加库目标，导出 TUI 入口、session runtime 与既有选项；二进制改为薄封装，行为与终端验收保持不变。
2. 新增一个通用 hook trait，全部方法可选，默认为空：

   ```rust
   pub trait TurnHooks {
       fn augment_system_prompt(&mut self, _prompt: &mut String) {}
       fn on_turn_end(&mut self, _outcome: &RunOutcome) {}
       fn on_event(&mut self, _event: &AgentEvent) {}
       fn filter_text(&mut self, _text: &mut String) {}
   }
   ```

3. `augment_system_prompt` 只能追加，不能替换基础 persona；`on_turn_end` 在最终 assistant 消息落盘后调用；`on_event` 只观察，不写日志。
4. 未使用的 hook 不产生额外请求、事件或 prompt token；hook 不拥有持久状态，事件日志仍是唯一真相。
5. 库 API 不出现身份、记忆、伴侣等产品类型；niuse 的 `TurnHooks` 实现留在 niuse。
6. 实施顺序：先加库目标与空 hooks（行为不变），再把 renderer 从 niuse 迁回，最后让 niuse 的二进制只做装配。任何一步失败都可停在上一步。

## 后果

**变好的：**

- niuse 可以复用 TUI、审批预览和终端验收，不再维护第二套前端。
- 其他嵌入方得到同一条路径；niucode 的终端能力只有一处实现。
- hook 面很小，必要时可以删除或替换，不影响 loop、事件或工具。

**变差的：**

- `niucode-cli` 的公共 API 需要维护；hook 的调用时机一旦承诺就不好改。
- 二进制与库的边界需要测试覆盖，否则容易出现“库能用、CLI 行为变了”。
- M6 的终端所有权工作会被拉长；需要一个新的验收项。

**接受的取舍：** 先付出一次结构成本，换掉长期的双前端维护。若 hook 最终只服务 niuse 一个调用方，仍应保留通用形状，避免内核被产品需求污染。

## 备选

| 方案 | 否决理由 |
|---|---|
| niuse 继续维护自己的 renderer | 每一条 TUI 改进都要重做两次；审批与 diff 会逐渐分叉 |
| 把 niuse 并入 niucode monorepo | 改变 niucode 的定位与发布节奏；见 niuse ADR-0003 |
| 给 niucode 加 `--profile personal` | 把产品藏进内核，内核被迫理解身份与记忆 |
| 只暴露事件流，让 niuse 自建 TUI | 等于把前端复制一次，没有解决根因 |

## 实施草图（第一步已落地）

第一步不改变上面的决策，只把「接受之后第一小步做什么」拆到可评审的程度。`src/lib.rs` 现在承载 `run()`，`src/main.rs` 只剩 `#[tokio::main]` 封装；`TurnHooks` 的默认方法仍是空操作，exec 与 TUI 路径都已接线，renderer 仍未迁移。

### 1. 结构

- `niucode-cli` 新增 `src/lib.rs`，把 `main.rs` 里的 `main()` 主体移成 `pub fn run() -> ExitCode`；bin 只保留一行调用。
- 模块声明迁入 lib；所有 `crate::` 路径保持原样。这一步只移动、不改行为，单独一个 commit，可单独 revert。
- 库目标不改变现有二进制、安装方式和终端验收。

### 2. API

```rust
pub struct TuiOptions {
    // session path, cwd, policy, approval, config
}

pub trait TurnHooks {
    fn augment_system_prompt(&mut self, _prompt: &mut String) {}
    fn on_turn_end(&mut self, _outcome: &RunOutcome) {}
    fn on_event(&mut self, _event: &Event) {}
}

pub fn run_tui(options: TuiOptions, hooks: Box<dyn TurnHooks>) -> Result<()>;
```

- `augment_system_prompt` 在构造 `AgentConfig` 之前调用一次。
- `on_event` 由运行时观察者转发 `AgentEvent`，只读，不写日志；持久化的 `Event` 仍是唯一真相。
- `filter_text` 在显示前修改一个 text delta；持久化的 `Event` 不变。它是嵌入方剥离自有协议块的 seam。
- `on_turn_end` 在最终事件落盘后调用。hook 不应 panic；库不额外捕获。
- `TuiOptions` 只包含现有入口已经需要的字段，不引入产品类型。

### 3. 验收

- 现有测试不改，`cargo test --workspace` 绿。
- 空 hooks 的路径与当前行为一致：至少覆盖一次非交互 run 和一次 TUI 启动/退出。
- 二进制仍能独立运行，`--help`、`--version` 与当前一致。
- niuse 的契约测试在 pin bump 后通过；没有 hook 被使用时，不应多出请求、事件或 prompt token。

### 4. 回滚

第 1 步（库目标）与第 2 步（hooks）分开提交。任一步出问题都能回到纯 bin，不影响已发布版本和事件日志。

### 5. 不在这一步

- 不迁移 niuse 的 renderer。
- 不发布 crate、不改依赖版本策略。
- 不改事件格式，不加第二份持久状态。
- 不为了让 hook 好用而增加内核概念。
