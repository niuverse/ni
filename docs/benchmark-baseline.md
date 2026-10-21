+++
id = "benchmark-baseline"
title = "M7 首轮基线：96 次四方配对"
kind = "doc"
status = "living"
order = 36
summary = "2026-10-21：四方双轨、八个自建题与四个外部题，完整分母、两次失败及后续优先级。"
+++

## 结果

2026-10-21，固定 DeepSeek `deepseek-flash`、thinking enabled/high、8192 输出上限，
四个 harness × 两轨 × 十二题 × 每配置一次，共 96 次正式运行。
两组任务分开报告；没有择优重试。另保留 34 次开发接入尝试，不纳入成绩。

| Harness | 自建基础 | 自建增强 | Polyglot 基础 | Polyglot 增强 |
|---|---:|---:|---:|---:|
| niucode | 8/8 | 8/8 | 4/4 | 3/4 |
| mini-SWE-agent | 8/8 | 8/8 | 4/4 | 4/4 |
| DeepSeek Harness | 8/8 | 8/8 | 4/4 | 4/4 |
| Pi | 7/8 | 8/8 | 4/4 | 4/4 |

这说明 niucode 能完成这组简单修改，且存在实际的生成结束处理缺口。
样本小、每题仅一次、公开外部题可能已进入训练数据，不能据此宣称总体胜负或行业水平。
两轨都没有实际触发父 agent 的委派工具调用，因此不能据此评价多 agent 能力。

## 两次失败

1. `test-py-csv-pi-basic-0`：模型达到 8192 输出 token，全部为推理，
   原始轨迹记录 stopReason=length；未新增 summarize，进程正常退出。
   原始回归通过，新需求的三个测试均因函数缺失报错。
2. `test-polyglot-py-transpose-niucode-enhanced-0`：单次输出 8192 token，
   reasoning_tokens=8192，源文件仍是原始 stub；CLI 报告 done，上游 12 个测试全部失败。
   niucode 没有保留服务端 finish_reason，不能从本条轨迹直接引用其值。
   代码检查确认 agent 丢弃 `StreamEvent::Done` 的 finish_reason，值得优先修复。

两次均保留原始结果；没有人工追加提示、修改验收或挑选重跑结果。
普通进程退出与模型报告完成都不代替独立评分。

## 资源开销

下表仅为完整八题自建集，费用包含失败尝试。中位耗时含容器启动和全部工具循环。
模型网关串行处理请求，不能把该耗时解释为不受限的并行吞吐性能。

| Harness | 轨道 | 中位秒数 | 请求数 | 输入 token | 输出 token | 费用上界 CNY |
|---|---|---:|---:|---:|---:|---:|
| niucode | 基础 | 19.95 | 64 | 335852 | 36999 | 0.967696 |
| niucode | 增强 | 25.77 | 102 | 708552 | 52056 | 1.833552 |
| mini-SWE-agent | 基础 | 22.63 | 64 | 347968 | 39536 | 1.012224 |
| mini-SWE-agent | 增强 | 30.58 | 65 | 315414 | 40421 | 0.954196 |
| DeepSeek Harness | 基础 | 21.69 | 66 | 465386 | 37462 | 1.230468 |
| DeepSeek Harness | 增强 | 29.45 | 88 | 831513 | 48869 | 2.053978 |
| Pi | 基础 | 12.07 | 45 | 157076 | 23948 | 0.505736 |
| Pi | 增强 | 18.75 | 72 | 388255 | 31263 | 1.026614 |

全部 API 请求费用上界 **¥14.219348 / ¥300**，包含开发调试 ¥1.178410。
没有缺失最终 usage 的请求，没有发生付费但缺少结果记录的运行。
按 [DeepSeek 官方高峰价](https://api-docs.deepseek.com/zh-cn/quick_start/pricing/)
计算，忽略缓存与非高峰折扣，因此不是实际账户扣费，实际费用排序也可能受折扣影响。
原生 CLI 显示的价格不用于此表。

niucode 增强轨在自建题上保持 8/8，费用上界约为基础轨 1.89 倍。
这一观察支持后续做验证流程的重复对照，不能由一次尝试认定增强流程无效。

## 可复核证据

- [全部结果、分母及资源表](../benchmarks/evidence/2026-10-21-v1/results.md)
- [机器可读汇总](../benchmarks/evidence/2026-10-21-v1/summary.json)
- [96 次配对审计](../benchmarks/evidence/2026-10-21-v1/audit.json)
- [原始轨迹、候选源码、补丁与独立评分](../benchmarks/evidence/2026-10-21-v1/attempts.tar.gz)
- [实际编译使用的 niucode 源码快照](../benchmarks/evidence/2026-10-21-v1/niucode-source.tar.gz)
- [产物哈希](../benchmarks/evidence/2026-10-21-v1/SHA256SUMS.json)
- [重跑说明与方法限制](../benchmarks/README.md)

框架固定版本：mini commit `04d809ceab9df28f9adaed044884180159172930`，
DSH `0.2.1-rc.2`，Pi `1.3.0`；niucode 为当前工作区源码快照，
完整文件哈希和二进制哈希见 toolchains.json。不是仅用版本号标识未提交代码。
模型服务别名不能冻结底层权重。DSH 使用 Messages，其他三方使用 Chat Completions；
原生系统提示和工具协议保留，增强轨是固定验证流程而非各家的最优配置。

外部题来自 [Aider Polyglot](https://github.com/Aider-AI/polyglot-benchmark)
固定 revision 的四题便利子集，原始测试可见，完整工具循环最多 40 次请求；
不是 Aider 官方榜单协议。SWE-bench 没有在本轮运行。

## 项目进度与下一阶段

M0–M5 内核里程碑完成；M6 功能已实现、仍在审阅，真实 Linux/macOS/Ghostty 与压力验收开放。
本轮 headless 成绩不关闭 M5-003、M5-006 或 M6-009 的终端 gate。

本次 M7-001 至 M7-004 完成：契约、案例、执行设施与首轮基线。
后续按证据推进：

1. M7-007：识别并记录生成截断，不能把未落地实现当作普通结束；续写如启用，必须有预算和副作用约束。
2. M7-005：恢复、取消、工具故障、写冲突及压缩保留约束的确定性专项；与实机终端验收分别记录。
3. M7-006：新 campaign 预注册至少三次重复，扩充跨文件与约束组合题，验证增强流程的质量和开销。

已通过 190 个 workspace 测试、5 个 Python runner/audit 测试、fmt、clippy；
八个自建参考修复全部通过，16 个错误修复被拒绝，四个外部 stub 失败且参考解通过。
最终 dashboard 与证据校验见交付记录。结果已在仓库准备好，尚未推送或发布到外部平台。
