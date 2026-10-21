+++
id = "adr-0030"
title = "ADR-0030：生成截断不等于任务完成"
kind = "adr"
status = "accepted"
order = 57
summary = "记录 finish_reason；length 复用现有错误路径，不执行截断调用、不自动续写。"
+++

## 决策

Accepted · 2026-10-21。用户要求修复输出截断却报告 done，保留原评测基线。

1. Loop 保留 provider 的 `finish_reason`，写入 `turn_end` 的可选同名字段。
   旧日志缺省为 None；TurnEnd 仍表示一次响应结束，不表示任务成功。
   core 仅保存这一持久事实，不解释 provider 协议或增加任务状态存储。
2. `length` 是不完整响应。消费本次流以收集最终 usage，记录已经收到的
   text/reasoning 与 usage，然后返回明确错误。该响应的工具参数即使是完整 JSON
   也不派发；不完整参数不经过 JSON 组装，不写成可执行的历史 tool_calls。
   若需要原始 tool deltas，使用已有 `--record` provider fixture。
3. 复用 runtime 的 `task_finished.status=error`，不发 TaskDone；CLI 输出 `error`
   并非零退出，TUI 显示同一个错误。`--goal` evaluator 的 length 也不得产生 met。
4. 不增加自动续写、重试配置或费用循环。已有已完成工具副作用保持，恢复由用户
   检查记录后发起；正常 stop/tool_calls、取消和流错误遵循现有路径。
5. 基线 v1 的镜像、失败记录、分数、case/adapter/runner/grader hashes 不改。
   确定性重建对照和新的实测另存；原日志未记录的 length 必须标明为注入条件。

## 后果

一次响应结束与任务完成由同一 Loop 判定，不让 CLI/TUI 各自猜测 token 上限。
测试通过 run、run_interactive 和真实 CLI 入口覆盖，另检验旧事件解码兼容。
此修复不保证模型能在输出预算内解题，也不把未知 finish_reason 推断为 length。
预算内的解题稳定性与后续更广泛的恢复机制继续独立评测。
