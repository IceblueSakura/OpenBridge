# OpenAI Vector Stores 调研

## 来源、范围与快照

本文只记录 Vector Store resource、file membership 与 processing lifecycle。Responses File Search tool 只引用该 resource，不在本文
定义 tool request/result。

- 官方来源：[Vector Stores API](https://developers.openai.com/api/reference/resources/vector_stores)
- 原始资料复核日期：2026-08-04；本次结构整理未重新在线复核 limit、billing、retention 或 status enum。

## 1. Resource lifecycle

Vector Store 是独立 opaque resource，可执行 create、list/retrieve、update/delete，并管理 file 或 file-batch membership。关联 file 后
可能经历 processing；完成前不能假定检索可用。

## 2. Identity 与状态

vector-store id、file id、membership/batch id 与 processing status 彼此不同。delete 是否级联、失败 membership 如何恢复、并发更新和
retention 都必须由正式 contract 说明。

## 3. 成本与数据边界

hosted indexing 可能产生存储、processing 与 retention 成本。原始 file、索引状态、resource id 与 metadata 可能敏感，不应进入低
基数之外的 telemetry label。

## 4. 证据边界

- Vector Store ready 不证明任意 model 或 File Search tool 有访问权限；
- create success 不证明 processing terminal、search quality 或 delete cascade；
- mock lifecycle 不证明真实索引、费用、retention 或长期运行。
