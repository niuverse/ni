+++
id = "M6-003"
title = "Single terminal owner and stable native transcript"
status = "review"
milestone = "M6"
module = "tui"
size = "L"
priority = "P1"
updated = "2026-10-21"
depends_on = ["M6-002"]
docs = ["pairing-upgrade", "adr-0028"]
+++

## 范围

- 建立唯一终端输入/输出所有者，统一活动区、transcript、菜单、审批和详情的模式转换。
- 迁移 guards 与直接打印；保持原生 scrollback，Ghostty fallback 在实测前保留。

## 验收

- [ ] 流式输出与输入/resize/menu 交错时不覆盖正文、不丢草稿，布局和 PTY 测试可复现。
- [ ] 所有交互终端写入经单一入口，后台 observer/任务不直接打印；输出和刷新有界。
- [x] 光标查询超时不吞用户按键；非 TTY、NO_COLOR、能力不足有明确行为。
- [x] 菜单关闭后原生选择复制恢复；审批/详情嵌套退出恢复进入前终端状态。
- [x] 正常退出、取消、错误路径均恢复 raw/mouse/paste/cursor；输入任务可取消和 join。
- [x] 原始控制序列在回答、工具、reasoning、路径等输出中统一清洗。
- [x] 终端特定例外有最小复现、回归和移除条件；删除被替代渲染路径。

- [x] 输入竖线与菜单焦点分离；inline 清屏后光标回到 viewport 起点，后续普通输出不覆盖 transcript；TestBackend / PTY 回归。

## 记录

2026-10-21：TerminalSession/DetailScreen 与 live 集中活动终端所有权，无变化帧去重；统一 crossterm 版本与 level-triggered 输入。PTY 已覆盖 linear/inline、resize、粘贴、审批、取消、详情与模式恢复；原生复制/字体/IME/真实 resize 待 M6-009。

实现须遵守 pairing-upgrade 中的 robust/clean 审查门槛及仓库 Definition of done。


验证索引：`docs/terminal-acceptance.md`。review 表示代码与本地回归可审阅，未宣称全部平台验收完成。

2026-10-21 补充：herdr 0.8.2 独立离线会话复现 inline 清屏保留旧列导致命令结果错位，修复交接起点；不增加 herdr 品牌分支。GUI 字体/IME 验收仍开放。
