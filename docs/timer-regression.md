+++
id = "timer-regression"
title = "M6-018：定时器竞态定位与依赖约束"
kind = "doc"
status = "living"
order = 36
summary = "用独立 interval 复现定位 Tokio 1.54.0 的析构同步缺失；固定 1.53.3 并保留升级回归。"
+++

## 根因与修复范围

TUI 使用 `Interval` 定期处理输入和刷新状态。在其他 ready 分支不断唤醒任务时，
前台可能在后台驱动完成 waker 操作前观察到 timer 已到期。

Tokio 1.54.0 的 `Sleep::reset_without_timer` 直接将内部 timer 设置为 `None`。
该版本将取消职责从 timer 析构迁到 Sleep 析构，移除内部 timer 的路径却没有取得
驱动锁来等待后台完成。重用该内存可与后台的 `AtomicWaker::take_waker` 竞争。
依据为本地版本源码、独立失败复现和只补回同步的对照实验；不是上游已确认的结论。
相关上游变更是 [tokio-rs/tokio #8458](https://github.com/tokio-rs/tokio/pull/8458)。

1.53.3 的 `TimerEntry` 析构仍执行 `cancel()`，通过驱动锁建立同步。因此项目在
Cargo.toml 和 Cargo.lock **精确固定 Tokio 1.53.3**。仅改 lock 不够，重新解析依赖时
可能再次选择问题版本。没有关闭 debug assertion，没有把 runtime 改成单线程，
没有修改终端交互或将诊断用补丁带入依赖。

## 因果对照

独立程序只依赖 Tokio，交替 poll 1ms interval 与 `yield_now()`，等待 5000 次 tick。
不含 niucode、PTY、文件读取、模型调用或网络。实验源和结果在
`docs/evidence/timer-2026-10-21/`，仓库对应的独立测试是
`crates/niucode-cli/tests/timer.rs`。

| 构建 | 结果 |
|---|---|
| 原版 1.54.0 debug，三次独立运行 | 两次 SIGABRT、一次 SIGSEGV，均约 1.1s 失败 |
| 原版 1.54.0 release | 完成 tick 后仍以堆损坏 SIGABRT 退出 |
| 1.54.0 临时副本，仅在移除 timer 前调用 `cancel(driver)`，三次运行 | 全部正常结束 |
| 新增仓库回归，原版 1.54.0 | 测试进程 SIGABRT |
| 同一仓库回归，固定 1.53.3 | 通过 |

这解释了为什么此前 release 长测通过仍不足以关闭问题：竞态不是每次触发，
release 没有断言也仍可能发生内存损坏。历史测试产物保留，不改写为失败或修复后的结果。

重跑独立实验时，在仓库之外建立最小 Cargo 项目，复制 evidence 中的 `reproducer.rs`
到 `src/main.rs`，依赖配置如下；切换到 `=1.53.3` 后重跑同一个程序即可比较。

```toml
[package]
name = "timer-repro"
version = "0.0.0"
edition = "2021"

[dependencies]
tokio = { version = "=1.54.0", features = ["rt-multi-thread", "macros", "time"] }
```

分别使用 `cargo run`、`cargo run --release`，检查完整进程退出状态。程序打印
`5000 ticks passed` 后仍可能在 runtime 析构时失败，不能仅凭该文字判定成功。

## 验证与后续升级

```sh
cargo test -p niucode --test timer
cargo test --release -p niucode --test timer
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo xtask dashboard --check
python3 crates/niucode-cli/tests/volume.py target/debug/niucode 100
python3 crates/niucode-cli/tests/volume.py target/release/niucode 100
python3 crates/niucode-cli/tests/pressure.py target/release/niucode 60
```

回归使用真实线程和时钟，不使用 paused clock；它能捕获已复现的竞争，但通过一次
不是所有调度均安全的证明。解除精确版本约束前需确认上游修复覆盖 timer 移除时的
驱动同步，再重复运行独立回归、debug/release 100 轮和取消/详情检查。
本次不触碰旧 benchmark 评分、失败尝试、计费记录或 M5/M6 实机验收门槛。

## 本次验收结果

210 项 workspace 测试和 debug/release 独立 timer 回归通过；fmt、clippy
`-D warnings`、dashboard check、5 项 benchmark 单测、8 个合成题和 4 个外部题
离线自检通过。58 个冻结的 benchmark 证据文件 SHA-256 未变，没有模型 API 调用。

三次 debug 100 轮与一次 release 100 轮均正常结束；包括取消、截断错误和日志正文复用。
首个 debug 进程启动后清理了上轮隔离 checkout 残留的 auth 构建缓存，因此首轮和后两轮
产物 hash 分开记录；三次进程实际 hash 均已核对，运行时源文件没有改变。
release 的 60 秒流式检查包含 4 次详情切换，取消到空闲耗时 3.09ms，终端状态恢复通过。
这些测量不是新的 30 分钟耐久认证，也不替代真实终端验收。

构建信息、逐次结果与失败证据见 `docs/evidence/timer-2026-10-21/`。
本机 `~/.local/bin/niucode` 已更新到本次 release，SHA-256 为
`da8682313c0314ca9165496ef72f8c9bfc5d7ec3d4acf983313ac28e5af9264d`；
旧产物备份为 `~/.local/bin/niucode.bak-20261021-171816`，新开进程即可使用修复版。
