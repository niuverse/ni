+++
id = "M1-003"
title = "Diff 预览与输出折叠"
status = "ready"
milestone = "M1"
module = "tui"
size = "M"
priority = "P2"
updated = "2026-10-20"
depends_on = ["M1-001"]
+++

## 范围

- 统一 diff 渲染：文件头、hunk、增删行、上下文折叠。
- 工具输出超过阈值时折叠，只显示摘要和"展开"提示。
- 审批前展示本次写入的完整 diff。

## 验收

- 大 diff 不刷屏，能按文件跳转。
- 行号与新增/删除标记对齐。
- 终端宽度变化后重排不破版。

## 记录

diff 是审批体验的核心。模型说"改好了"不算，用户看到 diff 才算。
