+++
id = "M7-007"
title = "Distinguish generation limits from successful completion"
status = "done"
milestone = "M7"
module = "loop"
size = "M"
priority = "P1"
updated = "2026-10-21"
completed = "2026-10-21"
depends_on = ["M7-004"]
docs = ["benchmark-plan", "benchmark-followup", "adr-0029", "adr-0030"]
+++

## 证据与范围

首轮 `test-py-csv-pi-basic-0` 在 8192 输出 token 内只产生推理，stopReason=length，
正常退出但没有新增 summarize，独立验收失败。此例不证明 Pi 总体较弱。

检查 niucode 也发现同类风险：provider 已传递 finish_reason，agent 的
`StreamEvent::Done { .. }` 分支丢弃它；没有 tool call 时按普通完成路径退出。
随后 `test-polyglot-py-transpose-niucode-enhanced-0` 也实测出现：
单次输出 8192 token 且 reasoning_tokens=8192，源码仍是原始 stub，CLI 报告 done，
12 个上游测试全部失败。该日志没有保留服务端 finish_reason，因此不能直接引用其值；
可以确认的是达到输出上限后没有落地实现却走了普通结束路径。
正式 campaign 中保持二进制冻结，后续独立修复与版本对照。

- [x] scripted provider 复现只有推理/空正文且 finish_reason=length 的结束。
- [x] 截断不能报告为普通任务完成，CLI/TUI 与持久事件的可见状态一致。
- [x] 不选择自动续写；不新增费用循环，不执行截断响应的工具调用。
- [x] 验证正常 stop、tool_calls、取消与流错误保持正确；ADR-0030 记录新语义。
- [x] 相同截断 fixture 做新旧版本对照，并以新镜像独立复测原失败题；不覆盖或混入 v1。

195 个 workspace 测试通过。对照旧版 exit=0/done、新版 exit=1/error；
同题新实测 12/12 通过。证据见 benchmark-followup；四方重复对比仍属 M7-006。
