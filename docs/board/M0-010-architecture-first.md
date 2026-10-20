+++
id = "M0-010"
title = "Architecture-first dashboard 改造"
status = "done"
milestone = "M0"
module = "dashboard"
size = "M"
priority = "P1"
updated = "2026-10-20"
completed = "2026-10-20"
depends_on = ["M0-003"]
docs = ["adr-0003"]
+++

## 范围

- 默认视图改为 Architecture，导航改为 Architecture / Work / Docs / Decisions。
- 架构视图移除进度条、完成率和卡片计数；左栏改为按层分组的模块导航。
- Work 保留卡片和里程碑筛选，但不再展示进度统计。
- Decisions 只收 ADR 与 `kind = "decision"` 事件。
- 模块聚焦：选中后高亮相关边、弱化无关模块；不做 zoom/pan。
- 视觉继续走 technical field guide，不引入前端框架。

## 验收

- [x] 打开 `docs/index.html` 第一屏是架构图。
- [x] Architecture 页没有进度条、完成率、Kanban 统计。
- [x] Work 仍可筛选里程碑、打开卡片详情。
- [x] Decisions 只显示决策事件和 ADR。
- [x] 模块聚焦、源码/ADR 联读、明暗主题正常。
- [x] 单文件离线，`cargo xtask dashboard --check` 通过。

## 记录

进度信息不是被删除，而是从第一屏移走：卡片仍在 `docs/board/`，Work 仍可查。看板重新成为架构阅读面。
