+++
id = "M6-004"
title = "Event-driven session runtime with cancellable approval"
status = "review"
milestone = "M6"
module = "loop"
size = "L"
priority = "P1"
updated = "2026-10-21"
depends_on = ["M6-001"]
docs = ["pairing-upgrade", "adr-0028"]
+++

## 范围

- 落实 ADR-0004 的进程内 Command/Event runtime，统一 TUI 与 exec 的执行、审批、取消和日志生命周期。
- core 仅新增确实需要的持久事件/字段；保持旧日志、TurnEnd 和 ToolResult.ok 的解释。

## 验收

- [x] 运行时为父 session 的唯一 writer；无第二份 durable queue/history，无 UI 直接日志追加。
- [x] provider/tool/approval 等待时仍处理取消和命令；有界通道不让慢 UI 阻塞取消。
- [x] 审批按 request/call ID 关联，重复/迟到答复不授权其他操作。
- [x] 取消清理工具进程组与子任务；副作用后未记结果时标记未知，不自动重复执行。
- [x] 旧日志、resume/fork/compaction 和 exec JSON 行为有回归；replay/fork 不执行工具。
- [x] 通过公共 runtime 接口测试流中错误、日志追加失败、client 断开与关闭顺序。
- [x] 迁移调用方并移除重复执行/审批/取消路径，不保留空 handler 或同步阻塞桥。

## 记录

2026-10-21：Command/Event runtime 与 headless callback 共用引擎；追加先于确认，有界命令/事件与独立取消通道，审批 request ID 防迟到/重复授权。回归覆盖等待、断开、背压、日志失败、旧 projection/exec/subagent。

实现须遵守 pairing-upgrade 中的 robust/clean 审查门槛及仓库 Definition of done。


验证索引：`docs/terminal-acceptance.md`。review 表示代码与本地回归可审阅，未宣称全部平台验收完成。
