+++
id = "M6-005"
title = "Live steering, queued tasks and draft preservation"
status = "review"
milestone = "M6"
module = "tui"
size = "L"
priority = "P1"
updated = "2026-10-21"
depends_on = ["M6-003","M6-004"]
docs = ["pairing-upgrade", "adr-0028"]
+++

## 范围

- 运行中 Enter 补充当前任务；显式排队提交独立任务；输入草稿跨详情、审批和取消保留。
- 接收状态来自 runtime 事件；队列恢复从日志投影，不维护本地会话数据库。

## 验收

- [x] 提交原文和消息 ID 成功落盘后才显示待接收；下一请求包含消息后显示已接收。
- [x] 当前 tool-call batch 结果配对完整后注入，补充保持顺序且无重复 user 消息。
- [x] 最后响应结束竞态不丢补充；已完成任务之后的输入明确进入下一任务。
- [x] 独立任务 FIFO，不提前注入当前上下文；队列满明确拒绝且保留草稿。
- [x] 流失败/取消/审批/详情切换保留草稿和待处理消息；恢复后排队任务不自动执行。
- [x] 输入、取消、排队、模式切换不会隐式改变权限或接受审批。
- [ ] scripted provider 的请求断言与 PTY 交互共同覆盖以上行为，无固定 sleep 依赖。

## 记录

2026-10-21：Enter steering / Alt-Enter FIFO queue 接通，InputSubmitted/InputApplied 为唯一恢复来源；拒绝保留原文，取消保留草稿和已接受待处理消息。scripted 请求断言与 PTY 已通过；长时间交错压力验收仍开放。

实现须遵守 pairing-upgrade 中的 robust/clean 审查门槛及仓库 Definition of done。


验证索引：`docs/terminal-acceptance.md`。review 表示代码与本地回归可审阅，未宣称全部平台验收完成。
