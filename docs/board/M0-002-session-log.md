+++
id = "M0-002"
title = "事件模型与 JSONL 会话日志"
status = "done"
milestone = "M0"
module = "session"
size = "M"
priority = "P0"
updated = "2026-10-20"
completed = "2026-10-20"
docs = ["adr-0002"]
+++

## 范围

- `Event` 枚举覆盖 session、user、assistant、tool result、turn end、compaction。
- `SessionLog` 只追加，每条事件立即 flush。
- `load` 逐行解码，坏行变成 warning，不丢整份历史。
- 单测覆盖 round-trip、坏尾行、空行。

## 验收

- [x] 写入后重新加载，事件序列完全一致。
- [x] 最后一行损坏时，前面的历史仍可读。
- [x] `ni session show` 能打印事件类型和数量。

## 记录

时间戳统一为 Unix 毫秒。`compaction` 记录被替换的事件数量，后续 resume 用它判断投影边界。
