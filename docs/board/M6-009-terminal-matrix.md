+++
id = "M6-009"
title = "Linux and macOS terminal acceptance matrix"
status = "blocked"
blocked_reason = "No real display or macOS terminal environment; renderer/IME/copy matrix and slow-device/network-filesystem measurements remain unverified. Synthetic 30-minute PTY stress passed."
milestone = "M6"
module = "tui"
size = "L"
priority = "P1"
updated = "2026-10-21"
depends_on = ["M6-005","M6-006","M6-008"]
docs = ["pairing-upgrade", "adr-0028"]
+++

## 范围

- 建立真实 Linux/macOS 终端和 tmux 验收记录；自动化、PTY、实机各自证明其覆盖范围。
- Linux：Ghostty/Kitty/WezTerm/GNOME Terminal；macOS：Ghostty/Kitty/WezTerm/Terminal.app。

## 验收

- [ ] 每个终端直连及 tmux 均记录 OS/终端/tmux/字体/配置/尺寸/commit 和测试证据。
- [ ] 全部组合覆盖中文/IME/emoji、多行粘贴、长行、resize、流式输出、补充、排队、取消、审批、详情、自然复制和 scrollback。
- [ ] 长时间输出与慢终端不使内存/队列无限增长，取消仍可用；记录复现负载和测量。
- [x] CI 增加 Linux/macOS 可运行测试，Linux 专属 sandbox 测试明确隔离，不静默放宽权限。
- [ ] 真实 Ghostty gate 通过后才移除品牌 fallback，并以同一证据关闭 M5-003/M5-006。
- [x] 缺少真实环境的项保持未验证并记录原因；不以模拟器、截图解析或 CI 代替实机通过。
- [x] 完整仓库 fmt/test/clippy/dashboard 检查通过，文档列明已测版本与保证边界。

## 记录

2026-10-21：新增 terminal-acceptance 记录、Unix PTY 集成与 Ubuntu/macOS CI 配置。当前无 DISPLAY/WAYLAND_DISPLAY，macOS/各终端直连与 tmux 实机、慢终端长时间测量尚未验证。

实现须遵守 pairing-upgrade 中的 robust/clean 审查门槛及仓库 Definition of done。


验证索引：`docs/terminal-acceptance.md`。review 表示代码与本地回归可审阅，未宣称全部平台验收完成。

2026-10-21 第二阶段补充：M6-015 完成合成 PTY 30 分钟持续输出、120 次详情切换与退出恢复；
100 轮大型 fixture、取消时延和独立 tmux 捕获见 `docs/tui-upgrade-validation.md`。
这些补充不关闭本卡的真实终端/设备验收。

后续 debug 重复轮次中发现间歇性定时器 panic，M6-018 已定位为 Tokio 1.54.0
interval 的析构同步缺失，并固定 1.53.3。新的回归证据见 `docs/timer-regression.md`；
先前通过的 release 压力记录仍只代表其原构建，不替代本卡的实机验收。
