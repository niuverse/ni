+++
id = "M6-014"
title = "Reduce repeated snapshot content without losing task-start evidence"
status = "done"
milestone = "M6"
module = "loop"
size = "L"
priority = "P1"
updated = "2026-10-21"
completed = "2026-10-21"
depends_on = ["M6-011"]
docs = ["pairing-upgrade", "adr-0028", "adr-0031", "tui-upgrade-validation"]
+++

## 范围

已观察到 hello 单轮日志 8,568,094 字节，两份快照各含 3,789,249 字节正文。
分别测量枚举/哈希/正文/序列化/回放；比较日志内引用已有正文与缩减正文采集的覆盖代价，
选定最小实现并删除重复内容写入。保留任务开始基线，不通过猜测用户意图跳过证据。

## 验收

- [x] 先提供固定 synthetic fixture 的改动前后测量，首次采集和后续无变化轮次分列。
- [x] 初始目标：394 文件 fixture 建立基线后，无变化轮次的证据字节增量减少至少 90%；明确分母和采集覆盖，不能靠丢弃未知状态达标。
- [x] 文件指纹预算与正文预算分别说明；超限、取消、扫描期间变化的原因可从日志恢复。
- [x] 持久字段或覆盖契约变化先补 ADR 与 decision 事件；不新增快照数据库或第二份会话状态。
- [x] replay/fork/resume、缓存删除、坏尾行、取消、外部编辑、二进制和未知覆盖有回归；旧日志可读。
- [x] 任务开始基线、dirty/index 保留和检查新鲜度判断不退化；采集可取消，不阻塞终端输入循环。
- [x] workspace tests、fmt、clippy、dashboard check 通过；记录性能未达标项，不以测量缺失宣称完成。

## 实施记录

2026-10-21：实现与回归通过，详见 `docs/tui-upgrade-validation.md`。真实平台门槛继续由 M6-009 跟踪。
