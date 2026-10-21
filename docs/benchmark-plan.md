+++
id = "benchmark-plan"
title = "M7：公开 harness benchmark"
kind = "doc"
status = "living"
order = 35
summary = "四方双轨评测：固定 DeepSeek、公开小仓库、独立评分及 300 元预算。"
+++

## 目标

让 niucode 在日常已有项目修 bug、加小功能时更可靠。先取得真实差距证据，再改进。
比较 niucode / mini-SWE-agent / DeepSeek Harness / Pi；基础与增强分轨。
用户确认日期：2026-10-21。架构约束见 ADR-0029。

## 任务集

首版八个合成任务，Python/Rust 各四题，bug/feature 各四题：
配置优先级、分页、Unicode 截断、路径排除、任务筛选/JSON、CSV 分组、
dry-run、缓存 TTL。每题为独立小仓库，无第三方依赖，不允许引入新依赖。
不是从真实 issue 抽取的任务，不宣称代表行业编码水平。

每题含 task.md、seed、reference.patch、judge、两个错误修复样例。
基线已有测试通过且新验收失败；参考补丁全部通过；错误修复被独立测试拒绝。
另外的 dev/smoke 题只验证适配和费用记录，永不混入正式成绩。
外部轨采用 Aider Polyglot 固定 revision 的四题小子集：Python grade-school/transpose、
Rust word-count/variable-length-quantity，使用原始 Exercism 测试，Rust 包含 ignored 测试。
选择发生在模型解题前，依据为 Python/Rust 各两题、标准库可运行、规模简单。
这是便利子集，不是随机代表样本。测试对 agent 可见，允许完整工具循环，
因此不能与 Aider 官方两次尝试榜单直接比较。SWE-bench 暂后置：本机 Docker Hub 拉取不可达。

## 执行

1. 离线验证案例与评分器，检查超时和子进程清理。
2. 锁定框架 release/commit、依赖 lock、工具链和容器 digest。
3. 基础轨保留原生工具/提示词；增强轨加入同一通用验证流程，
   niucode/DSH 可用原生委派，Pi 使用固定官方 subagent 示例，
   mini 使用单 agent 验证流程。增强结果不是“最优调参后上限”。
4. 同一个 DeepSeek 模型，思考 high，统一输出/上下文配置、40 请求/900 秒上限。
   所有后续/子 agent 请求计入预算；API 请求由网关计费。
5. 独立 dev 题试跑，修正接入问题后冻结正式配置。随机固定顺序交错四方运行。
6. 正式任务一次运行只记一次结果，不挑最好一次；重复运行目标 3 次，
   受实测成本约束。未完成重复不能伪称稳定胜负。
7. 评分器只执行独立需求测试和原始回归测试。报告运行状态与 candidate 测试状态，
   超时后偶然通过的 patch 不计完整成功。
8. 汇总逐题胜负、成功率、费用上界、token、耗时、失败分类和产物路径。
   功能/恢复/终端可靠性与编码成功率分别解释。

## 预算与凭证

首轮总上限 ¥300，四框架、两轨道、调试、重试、审阅和委派全部包含。
默认使用 DeepSeek 官方 endpoint；AuthStore 按 env → keyring → 0600 文件解析。
用户允许将 ~/.zshrc 中的字面量 DEEPSEEK_API_KEY 导入启动环境；不执行 shell 配置，
不输出值、不写入代码、镜像或运行日志。

价格按 2026-10-21 官方高峰价保守计费：输入未命中 2 元/M、输出 8 元/M；
忽略缓存/非高峰折扣，因此报告费用为上界，不是账户账单。
每次请求在网关先追加并 fsync 预留，再调用 API；usage 完整才降低预留。
重启从计费日志恢复，损坏尾行拒绝继续。API 价格变更后必须重新核验价格与预算公式。

## 验收

- 八题 seed/reference/mutation 自检通过；有可重复命令和记录。
- 四方两轨接入试跑通过，配置冻结且能重跑。
- 至少一组完整配对正式结果，分母完整，未运行项不算失败也不隐瞒。
- 外部公开任务接入与上游原始测试评分有真实证据；与官方榜单协议差异明确记录。
- fmt/test/clippy/dashboard 检查通过。
- M6 实机 gate 仍开放；评测成绩不自动改变该状态。
- 比较结论不要求 niucode 获胜；后续优化由失败证据排序。

## 后续阶段顺序

1. M6-009 / M5-006：补真实 Linux/macOS/Ghostty 与长时间终端证据；保留 linear fallback。
2. M7-004：完成本次 96 次首轮基线和逐题审计；不在正式试跑中调参或修题。
3. M7-007 已完成：length 明确报错并保留结束原因；确定性前后对照及原失败题独立复测
   见 benchmark-followup。原 v1 基线未修改，四方重复对比继续在后续新 campaign 进行。
4. M7-005：加入恢复、取消、工具故障、压缩保留约束与 stale write 的确定性场景。
5. M7-006：在新 campaign 中做至少三次重复，并扩充能区分框架能力的组合任务。

首轮一次尝试用于建立基线；原先的三次重复目标保留为后续稳定性验收，
不把当前样本写成已完成三次，也不以简单题通过率判断日常大仓库可靠性。
