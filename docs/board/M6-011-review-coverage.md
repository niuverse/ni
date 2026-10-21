+++
id = "M6-011"
title = "Distinguish unknown coverage from proven workspace changes"
status = "done"
milestone = "M6"
module = "loop"
size = "M"
priority = "P0"
updated = "2026-10-21"
completed = "2026-10-21"
depends_on = []
docs = ["pairing-upgrade", "adr-0028", "adr-0031", "tui-upgrade-validation"]
+++

## 范围

修复 M6-008 审阅投影：两份无 digest 条目相同，不代表文件发生变化。
复现来自无工具调用的 hello，130 个预算外条目被误报为工作区改动。
已证实变化、可比较未变、无法确认分别表达；只有已证实变化才继续判断来源。
保留任务开始基线和现有事件日志，不修改 benchmark 基线。

## 验收

- [x] synthetic repo 重现 394 个条目中 130 个未知：相同前后快照不生成 130 个变化，未知覆盖仍可查询。
- [x] 不完整文件枚举中的缺失条目不自动断言新增/删除；完整覆盖下真实增删仍被识别。
- [x] 可比较的内容或模式变化保持可见；二进制、大文件和扫描期间变化的未知原因明确。
- [x] 已有 dirty/staged/untracked 不归为本轮成果；真实外部变化不被隐藏或归因 agent；index 不变。
- [x] 旧日志及新日志回放结论一致，缺失证据不伪造未变化或成功；无需新增持久状态。
- [x] focused regression、workspace tests、fmt、clippy、dashboard check 通过。

## 实施记录

2026-10-21：实现与回归通过，详见 `docs/tui-upgrade-validation.md`。真实平台门槛继续由 M6-009 跟踪。
