+++
id = "M6-002"
title = "Shared Unicode composer and completion behavior"
status = "review"
milestone = "M6"
module = "tui"
size = "L"
priority = "P1"
updated = "2026-10-21"
depends_on = ["M6-001"]
docs = ["pairing-upgrade", "adr-0028"]
+++

## 范围

- 提取共用编辑状态、字素导航和布局；迁移 linear/inline 到同一行为实现。
- 补全继续以 CommandRegistry 为来源，路径发现复用 ignore；删除重复编辑/历史/候选规则。

## 验收

- [x] 多行 CRLF 粘贴保持内容且不执行命令、不自动提交；一次撤销恢复原草稿。
- [x] CJK、组合字符、ZWJ emoji 编辑不拆字素；布局和光标使用同一显示列计算。
- [x] 长输入软换行、上下移动、Home/End、单词操作和撤销/重做有行为测试。
- [x] 历史浏览返回原草稿；补全针对光标 token，长候选列表滚动时选择项可见。
- [x] 嵌套 ignore 和大仓库路径发现可取消；不阻塞按键事件处理。
- [x] 两种呈现路径通过同一行为测试，删除已迁移的重复逻辑；不新增终端品牌分支。

- [x] `/model` / `/permissions` 打开菜单不改变配置，选择后一次确认生效；Tab 仅填入，Esc 取消；键盘/鼠标与粘贴命令有回归。

## 记录

2026-10-21：共享 Composer/Completion 已迁移两种呈现；字素/宽度、CRLF 粘贴单次撤销、历史草稿、光标 token 补全与嵌套 ignore 有回归，旧 inline 编辑器已删除。

实现须遵守 pairing-upgrade 中的 robust/clean 审查门槛及仓库 Definition of done。


验证索引：`docs/terminal-acceptance.md`。review 表示代码与本地回归可审阅，未宣称全部平台验收完成。
