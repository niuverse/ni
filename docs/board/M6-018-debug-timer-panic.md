+++
id = "M6-018"
title = "Diagnose intermittent debug-build timer panic under repeated TUI turns"
status = "ready"
milestone = "M6"
module = "tui"
size = "M"
priority = "P1"
updated = "2026-10-21"
depends_on = ["M6-015"]
docs = ["tui-upgrade-validation", "adr-0028"]
+++

## 观察

2026-10-21 清理回归中，debug 构建偶发在输出回答后崩溃。
一次 workspace 测试失败；同一构建重跑三个 PTY 场景及完整 workspace 均通过。
另外三次 100 轮 debug 检查，两次失败，一次完整通过，不能用成功重跑消除失败证据。

完整堆栈定位到 Tokio 1.54.0 的定时器驱动：

```text
tokio::sync::task::atomic_waker::AtomicWaker::take_waker
  atomic_waker.rs:324: assertion failed: old_state == WAKING
tokio::runtime::time::entry::StateCell::fire
tokio::runtime::time::entry::TimerHandle::fire
tokio::runtime::time::Handle::process_at_time
```

单独 100,000 次异步文件操作和 timeout 探针未复现，Tokio 对应源文件与本地
registry 归档一致。现有证据不够证明根因在 Tokio、项目调用或工具链；不据此
升级依赖、关闭断言、加重试或删除测试。此前的 release 压力记录保留其原构建和范围。

对照：另行构建 `5dbcecc` 的清理前实现，运行同一 100 轮场景也失败于该定时器路径。
因此该问题早于 `5071042` 的启动/测试壳清理；此对照不证明其具体根因或 release 安全。

## 复现

```sh
cargo build -p niucode
RUST_BACKTRACE=1 python3 crates/niucode-cli/tests/volume.py target/debug/niucode 100
```

使用本机合成 fixture，无网络或模型费用。失败与成功输出都应保留；失败时抓取完整
PTY transcript，避免只保留默认末尾 6000 字节而丢失堆栈起点。

## 验收

- [ ] 最小复现明确根因，并比较清理前/后及 debug/release 构建。
- [ ] 修复有失败前/成功后的证据，不通过抑制断言或弱化检查规避。
- [ ] 完整 workspace 与重复轮次验证通过；真实终端 gate 独立保留。
