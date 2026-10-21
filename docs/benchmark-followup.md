+++
id = "benchmark-followup"
title = "M7：截断完成状态修复与独立复测"
kind = "doc"
status = "living"
order = 37
summary = "原 v1 基线不变；相同截断输出旧版 done、新版 error；同题独立实测通过 12 项测试。"
+++

## 修复结果

ADR-0030：Loop 不再丢弃 finish_reason。length 保留正文、推理与用量后报错，
该响应中的工具调用不执行；CLI 非零退出、不报 done；交互 runtime 不发 TaskDone，
日志写 task_finished=error。Goal evaluator 同样拒绝截断判断。没有增加自动续写。

## 失败案例验证

从原失败的 Polyglot transpose 运行取末轮推理/用量，在新工作区用同一个 fixture
运行旧版和新版：旧版 exit=0/done，新版 exit=1/error；两者独立测试均为 0/12。
原日志没有记录结束原因，所以明确注入 length，不能把它称为原始网络重放。
这是结束判定的回归验证，不是把失败题分数改成成功。

另用新镜像、原任务/评分器和相同 DeepSeek 配置独立实测一次：12/12 通过，
12 次模型调用，43.814 秒。该次未触发 length，单次成功不证明解题率提升。
新增费用上界 ¥0.263596；共用原费用日志，累计上界 ¥14.482944 / ¥300。

完整证据在 `benchmarks/evidence/2026-10-21-truncation-fix/`。
`2026-10-21-v1` 的 20 个归档文件字节不变，原 96 次成绩和失败记录全部保留。

## 清理与验证

删除无消费者的 styled prompt 参数、唯一调用点的格式化分支与两个 Python import，
清理路径净减 11 行。未把整个 dirty 工作区当作无效代码删除。
临时架构报告列出后续两个候选：统一文本布局、收拢 history/approval 详情状态。
本轮未实施这两项结构重整，终端实机验收继续独立。

195 个 Rust 测试、5 个 benchmark 测试、fmt/clippy/dashboard 检查通过。
回归覆盖仅推理、截断/完整工具参数、CLI/runtime 成功通知、goal 判定和旧日志兼容。
