+++
id = "tui"
title = "线性 TUI 与审批"
kind = "doc"
status = "living"
order = 33
summary = "niucode tui：流式文本、折叠 reasoning、工具状态、单键审批、取消与 session 复用。"
+++

## 用法

```sh
niucode tui
niucode tui --cwd /path/to/repo
niucode tui --sandbox read-only --network off
niucode tui --approval ask
niucode tui --resume /path/to/session.jsonl
niucode tui --replay /path/to/fixture.json
```

默认策略与 `niucode exec` 相同：`workspace-write + network off + on-failure`。

## 交互

```text
niucode> find the failing test
thinking…
→ bash cargo test
← bash ok exit: 1 ...
…streaming answer…
turn usage: in=... out=...

niucode> exit
```

- 输入一行任务，Enter 提交；`exit` / `quit` / `:q` 退出。
- `Ctrl-C` 取消当前 turn；2 秒内再次 `Ctrl-C` 强制退出。
- reasoning 只显示 `thinking…`，不刷屏。
- 工具调用显示 `→ name summary`，结果显示 `← name ok/error`。
- 文本增量直接写到 stdout，首 token 到达即渲染。

## 审批

`--approval ask` 时，每个 `write` / `edit` / `bash` 前暂停：

```text
approval required: write src/main.rs
+fn main() {
+    println!("hi");
+}

[a]llow  [d]eny  [s]ession
```

- `a` 允许本次。
- `d` / Esc 拒绝本次。
- `s` 本次 session 内自动允许后续变更型工具。
- 终端不是 TTY 时退化为逐行 `a/d/s` 输入，方便管道测试。
- `write` 显示 `+` 预览；`edit` 显示 `-old` / `+new`；`bash` 显示命令。

## 取消与事件

TUI 调用与 headless 相同的 `run_with_history`，只消费 `AgentEvent`，不 import loop 内部。

- 取消会 drop provider stream 和工具 future；bash 的进程组由 `ProcessGroupGuard` 清理。
- 取消后 session 追加 `session_end`，已完成的事件保留。
- 每次运行仍有 `policy` / `approval` / `tool_result` 审计。
- 同一进程内复用 session 与 history；每次 turn 后从日志重新投影，避免 UI 自己维护第二份对话状态。

## 边界

- 线性输出，不是全屏 TUI；没有滚动区、面板和鼠标。
- 审批时 Ctrl-C 当前按拒绝处理，不是取消整轮。
- diff 预览是工具输入的 `+/-` 片段，不是完整文件 diff。
- 窄终端依赖终端自身换行；80 列可读，但不做自适应布局。
- 大输出仍受 bash 20KB 尾部预算约束。
