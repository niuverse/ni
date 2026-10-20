+++
id = "M0-003"
title = "离线项目看板 v0"
status = "done"
milestone = "M0"
module = "dashboard"
size = "L"
priority = "P1"
updated = "2026-10-20"
completed = "2026-10-20"
depends_on = ["M0-001"]
docs = ["readme"]
+++

## 范围

- 三栏壳：里程碑导航 / 画布 / 详情。
- 四个视图：Board、Architecture、Docs、Timeline。
- 数据源：`docs/dashboard/project.json` + 一卡一文件的 TOML frontmatter。
- 构建为单个 `docs/index.html`，无 CDN、无运行时 fetch，`file://` 可开。
- `--check` 校验引用、状态和生成物是否过期。

## 验收

- [x] 双击 `docs/index.html` 能看到真实 M0 卡片。
- [x] 架构图展示模块状态和依赖箭头。
- [x] ADR 与架构文档能在右栏渲染。
- [x] `cargo xtask dashboard --check` 在生成物过期时失败。

## 记录

浏览器只读，不提供拖拽回写。状态变更走仓库，避免看板变成第二个真相源。后续由 `M0-010` 改为 Architecture 优先。
