+++
id = "M6-015"
title = "Gate transcript volume and sustained pairing behavior"
status = "done"
milestone = "M6"
module = "tui"
size = "M"
priority = "P1"
updated = "2026-10-21"
completed = "2026-10-21"
depends_on = ["M6-012", "M6-013", "M6-014"]
docs = ["pairing-upgrade", "terminal-acceptance", "adr-0028", "adr-0031", "tui-upgrade-validation"]
+++

## 范围

汇总前序卡的回归，增加输出规模和持续结对的发布验收。fixture 从 M6-011 开始积累。
使用 synthetic repo、scripted provider 和 PTY，不复制私人会话正文，不调用付费模型。
真实终端证据继续由 M6-009 管理，不重建另一套实机门槛。

## 验收

- [x] hello/纯读取/1、100、1000 文件改动/长命令/检查失败/生成截断按计划断言终态、主屏行数和详情完整性。
- [x] 20/40/80/120 列及 Unicode/控制字符覆盖实际消费者，避免只断言 ANSI 或私有实现结构。
- [x] 100 轮和 30 分钟慢输出记录内存、日志增长、回放/采集/取消时延；队列有界，无重复完成及尾部丢失。
- [x] 受控正常等待场景 Ctrl-C 到停止输出 P95 ≤ 250ms；单列进程清理时间与不可中断 I/O 限制，记录机器/构建/负载。
- [x] 并发补充、队列、审批及详情开关不丢消息，取消后终端状态与输入草稿可用。
- [x] M6-009 收到真实终端复验步骤；无实机证据时 M5 Ghostty/M6 发布 gate 保持开放。
- [x] 原 benchmark 两组 evidence 的文件集与 hash 不变；workspace tests、fmt、clippy、dashboard check 通过。

## 实施记录

2026-10-21：209 个 Rust 测试、fmt、clippy、dashboard 检查通过；100 轮与 30 分钟测试的
固定产物 hash、字节/内存/时延见 `docs/tui-upgrade-validation.md`。100 轮后的任务结束
P95 124.9ms；输入框恢复 P95 350.28ms 单列，不冒充 250ms 内恢复。实机 gate 保留。
