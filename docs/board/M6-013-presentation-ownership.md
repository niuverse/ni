+++
id = "M6-013"
title = "Consolidate review presentation and detail view state"
status = "done"
milestone = "M6"
module = "tui"
size = "M"
priority = "P1"
updated = "2026-10-21"
completed = "2026-10-21"
depends_on = ["M6-012"]
docs = ["pairing-upgrade", "adr-0028", "adr-0031", "tui-upgrade-validation"]
+++

## 范围

沿用 M6-011 的事实区分，将审阅中的展示字符串收敛为结构化派生数据；CLI 统一文案与布局。
删除 summary/detail 重复拼接、live/tui 重复呈现责任及详情/审批重复派生字段。
只迁移实际消费者，不引入通用 renderer、额外事件循环或新的持久存储。

## 验收

- [x] agent 审阅投影不依赖终端宽度、颜色和控件；主屏及详情读取同一组事实。
- [x] 每种任务终态只有一个呈现责任点；详情不再重复打印相同路径和检查清单。
- [x] 详情/审批显式区分，已有 Composer/text_layout/TerminalSession 继续分别管理编辑、布局、终端模式。
- [x] 打开/关闭详情、审批嵌套、resize、Esc、Ctrl-C 及运行中输入不丢草稿/滚动位置，不误授权。
- [x] 调用方迁移与旧分支删除同次交付；CLI JSON、resume、fork、权限和默认 linear 回归通过。
- [x] workspace tests、fmt、clippy、dashboard check 通过；路径/边界变化时同步 project.json。

## 实施记录

2026-10-21：实现与回归通过，详见 `docs/tui-upgrade-validation.md`。真实平台门槛继续由 M6-009 跟踪。
