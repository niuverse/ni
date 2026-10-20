+++
id = "M4-011"
title = "Runtime /model and /permissions"
status = "done"
milestone = "M4"
module = "tui"
size = "M"
priority = "P2"
updated = "2026-10-20"
completed = "2026-10-20"
depends_on = ["M4-001", "M4-010"]
docs = ["adr-0015", "tui"]
+++

## 范围

- `/model` 显示当前模型和建议；`/model <name>` 切换当前 provider 内的模型。
- `/permissions` 显示当前 policy；切换 ask/review/full-access/ci。
- 两者都写入事件，下一次 turn 生效；`/resume` 恢复，`/clear` 保留，`/reset` 清除。
- `/status` 显示生效 model 和 permission profile。

## 验收

- [x] model/permission 选择事件可 resume；clear/reset 语义明确。
- [x] `/model` 不允许跨 provider，避免 endpoint/auth 错配。
- [x] permission profile 映射到现有 sandbox/network/approval，不新增执行层。
- [x] full-access 有显式警告；`/plan` 仍是独立 mode。
- [x] PTY 覆盖 model switch、provider guard、permissions、clear/reset/resume、status。

## 记录

- `ModelChange { model }` / `PermissionChange { profile, sandbox, approval, network }`。
- `PermissionProfile::{Ask,Review,FullAccess,Ci}`；`apply` 只改 AgentConfig 的既有字段。
- `apply_runtime_config` 在每个 turn 前应用 model/permission；goal 和 plan allowlist 仍在 TUI 层。
- `/model` 建议列表：`NIUCODE_MODEL_CHOICES` 或内置列表；无 virtual model/router。
- `/model` / `/permissions` 的参数以行编辑器下拉呈现；裸命令 Enter 显示当前值，方向键/鼠标/Tab 选择后填入。
- Pi 的 selection/dispatch 分离作为参考；v1 只记录选择，provider 继续使用 `CompletionRequest.model`。
