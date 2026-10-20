+++
id = "M0-011"
title = "niucode auth：keyring 与安全凭证解析"
status = "done"
milestone = "M0"
module = "auth"
size = "M"
priority = "P1"
updated = "2026-10-20"
completed = "2026-10-20"
depends_on = ["M0-004"]
docs = ["auth", "adr-0005"]
+++

## 范围

- 新增 `niucode-auth`：凭证解析顺序 env → OS keyring → 0600 文件。
- `niucode auth set <provider>` 隐藏输入，或 `--stdin` 供脚本使用。
- `niucode auth status` 只显示来源，不打印密钥。
- `niucode auth unset <provider>` 清理 keyring 和文件。
- `niucode provider test` 通过 `niucode-auth` 取 key，不再只读环境变量。

## 验收

- [x] `DEEPSEEK_API_KEY` 优先于 keyring 和文件。
- [x] keyring 可用时写入 keyring；不可用时回退 0600 文件。
- [x] 文件权限为 0600，状态对 insecure 权限给出警告。
- [x] 损坏的凭证文件返回无内容泄露的错误。
- [x] `status` / `provider list` 不打印密钥。

## 记录

Linux 的 keyring 走 Secret Service（需要 DBus）；无桌面/容器环境可设置 `NIUCODE_AUTH_DISABLE_KEYRING=1` 强制文件。这个卡片只解决 API key 的登录体验；ChatGPT 账号 OAuth（SIWC）和 DeepSeek 网页账号不在范围内。
