+++
id = "tui-upgrade-validation"
title = "TUI 第二阶段：实现与验证"
kind = "doc"
status = "living"
order = 35
summary = "hello 刷屏修复、结构化审阅、快照正文复用、语义高亮、活动反馈与可复现压力测试。"
+++

## 实现范围

2026-10-21：M6-011 至 M6-016。审阅把已证实变化、覆盖未知、命令检查分别表达；
主屏系统收尾最多三行，普通对话不打印缺少检查，成功工具调用仅更新活动进度。
任务结果由 live/presentation 负责，删除与 tui 的重复成功输出和 summary/detail 双重拼接。
历史和审批使用互斥视图；审批可以保存并恢复阅读位置。Theme 在清洗后统一生成两种模式的高亮。

快照保留任务开始语义；后续快照对相同 digest 省略重复正文，审阅从目标事件之前的日志恢复。
没有已有正文时明确无法恢复 diff。不新增 core 字段、持久数据库、依赖包或模型调用。

## 可重复验证

```sh
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo xtask dashboard
cargo xtask dashboard --check
cargo build --release -p niucode
python3 crates/niucode-cli/tests/volume.py target/release/niucode 100
python3 crates/niucode-cli/tests/pressure.py target/release/niucode 1800
```

`volume.py` 使用 264 个约 15.6 KB 的可读文件和 130 个超单文件预算的稀疏文件。
旧方式字节基准由相同快照恢复重复正文后计算；它是存储对照，不是新的模型 benchmark 分数。
100 轮后检查每次都只有一个完成结果、未知条目不进入主屏、正文仅存一次，再验证取消和生成截断。
时间测量区分输入 Ctrl-C 到任务结束事件，以及到空闲输入框；后者还包含审阅和上下文恢复。

`pressure.py` 只访问本机临时 SSE 服务，每 50ms 输出一行，PTY 消费者延迟 100ms，
定期打开/关闭详情。结束时取消、校验任务状态与 termios 恢复，记录内存和时延。
这不是对真实 DeepSeek 服务、网络文件系统或 GUI 字体的测试。

## 验证状态

209 个 Rust 测试通过；其中 Unix PTY 入口执行原交互矩阵、394 文件多轮场景、100 次真实文件写入和审批恢复阅读位置用例。fmt、clippy `-D warnings` 和 dashboard check 通过。
独立 tmux 的 40/80/120 列捕获验证回答/代码/详情没有覆盖或错行；捕获保存在 `docs/evidence/tui-2026-10-21/`，不是 GUI 认证。

原始 hello 会话的只读副本也通过新版 Ctrl-O 重放：没有已证实变化，130 个缺失 digest 的条目进入覆盖未知；原会话没有被修改。

100 轮 release 测量见 `docs/evidence/tui-2026-10-21/volume-100.json`：

| 指标 | 结果 |
|---|---|
| 首次完整轮次日志 | 4,379,855 字节 |
| 后续单轮日志最大增量 | 103,322 字节 |
| 同一 fixture 恢复重复正文的双快照 JSON 对照 | 8,662,982 字节/轮 |
| 后续单轮对照减少 | 98.81% |
| 任务开始到起点快照事件（含枚举/读取/哈希） | 中位数 20 ms，P95 35 ms |
| 100 轮后离线 replay CLI（含启动/解析/投影） | 64.48 ms |
| 100 轮后 Ctrl-C 到任务结束事件 | P95 124.9 ms |
| Ctrl-C 到空闲输入框 | P95 350.28 ms |
| 被测进程 RSS：首轮 → 百轮 | 24,348 → 49,456 KiB |

原计划的 250ms 指标针对停止执行/输出；空闲输入框恢复还包含审阅和上下文投影，因此单独报告且目前超过 250ms，不把它描述成即时恢复。RSS 包含回放 provider 的请求历史与日志投影，不单凭两点判断泄漏。
30 分钟流式/慢消费测试通过，见 `docs/evidence/tui-2026-10-21/pressure-1800.json`：
连续运行 1800.05 秒、120 次详情切换、7,556,048 字节终端输出；RSS 从 12,976 KiB 到
14,008 KiB（峰值相同，约 13.7 MiB）。结束时 Ctrl-C 到空闲输入框为 12.41 ms，
任务记录 cancelled，无成功收尾，退出后 termios 与启动前一致。本地 SSE 仅收到一次请求，没有付费模型调用。

30 分钟测试产物 SHA-256：`194d526a8a0a5fa30d2b0710cc0635270aa16cb49b896159ae663eeec64c0663`。
最终安装 SHA-256：`2e6381508bf227d4b0253025f77311a84d9cf5718927e2e9f058e437e8ba8a35`。
长测后仅补充错误原因的呈现归属：终端 guard 退出后统一显示错误，覆盖提前 I/O 失败；
流式处理、快照和活动状态算法未变化。该最终补丁重跑完整测试与 5 秒流式取消/恢复检查，
短测和长测分别记录产物 hash，不将长测结果冒充最终补丁的同构建测量。
旧安装备份：`~/.local/bin/niucode.bak-20261021-161723`。版本号仍为 0.1.0，应以产物 hash 区分。
M5-003、M5-006、M6-009 实机门槛保留。没有 DISPLAY/WAYLAND_DISPLAY 或 macOS 环境，不能宣称这些终端已通过。

原 `2026-10-21-v1` 和 `2026-10-21-truncation-fix` 共 58 个 benchmark evidence 文件保持原样，按 SHA-256 校验。

## 清理后的补充发现

2026-10-21：M6-017 删除启动残留并共用 PTY 测试入口。209 个 workspace 测试、
fmt、clippy 和 benchmark 离线自检通过，但期间出现 debug 构建的间歇性定时器 panic。
三次额外的 100 轮 debug 检查中两次失败、一次通过；不以通过的一次覆盖失败结果。
堆栈、复现命令和待验证假设由 [M6-018](board/M6-018-debug-timer-panic.md) 跟踪。
本发现不改写上文已有 release 构建的历史测量，也不表示当前已消除该问题。
