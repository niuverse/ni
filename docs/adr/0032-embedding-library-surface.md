+++
id = "adr-0032"
title = "ADR-0032：向嵌入方暴露可复用前端与 turn hooks"
kind = "adr"
status = "proposed"
order = 59
summary = "把 TUI 与 runtime 暴露为库，并提供通用 hook：prompt 追加、turn 结束、事件观察；不引入产品类型，不新增第二状态。"
+++

## 状态

Proposed · 2026-10-21。来自 niuse 的嵌入需求；上游讨论见 [#1](https://github.com/niuverse/niucode/issues/1)。等待维护者决定是否并入 M6 终端所有权工作。

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
       fn on_event(&mut self, _event: &Event) {}
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
