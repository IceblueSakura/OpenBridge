# Uploads、Vector Stores 与 File Search 实现细节

**目标状态：** 仅作协议参考，不在现阶段 1/2 实施范围。

## 为什么必须独立于 Files

这组 API 共享 File resource，但状态模型不同：

- Uploads 是临时、多 part、可 complete/cancel 的上传事务，完成后才产生普通 File；
- Vector Stores 是可创建、列出、更新、删除并附加文件的托管索引资源；
- vector-store file/file batch ingestion 是异步状态机；
- File Search 是 Responses 中消费 vector store 的 hosted tool，不是 `/files` 的查询参数。

官方当前说明 Upload 最多接收约 8 GB，并在创建约一小时后过期；这些值是当日 OpenAI profile 事实，实施前需复核。资料：[Uploads API](https://developers.openai.com/api/reference/resources/uploads)、[Vector Stores API](https://developers.openai.com/api/reference/resources/vector_stores) 与 [File search](https://developers.openai.com/api/docs/guides/tools-file-search)。

## Uploads 事务

核心操作是 create、add part、complete 和 cancel：

1. create 声明 filename、purpose、总 bytes 和 MIME，返回 `upload_id` 与 expiry；
2. parts 独立上传 bytes，每个 part 返回 `part_id`；
3. complete 提交有序 part IDs，服务拼接并返回最终 File；
4. cancel 终止未完成 Upload。

`upload_id` 和 `part_id` 都必须绑定 issuer、owner 与 expiry。part retry、重复 part、乱序 complete、complete-after-cancel、cancel-after-complete 和网络超时后的 unknown outcome 需要显式状态。普通 HTTP retry/fallback 无法替代事务语义。

若网关使用本地 ledger，至少记录状态、issuer、owner、expected bytes、part identity/order、expiry 和最终 file mapping；更新需要并发控制和 durable write。若只签名封装 IDs，则仍要从上游查询状态，并证明 credential scope 在 TTL 内稳定。

## Vector Store 生命周期

Vector Store 资源至少涉及 create/list/retrieve/update/delete、file attach、file batch、ingestion polling、file removal/content 和 search。关键边界：

- vector store、file、batch 三种 ID 可能各有 issuer；
- attach/ingestion 是有副作用的异步操作，HTTP 200 不代表内容可搜索；
- 客户端必须观察 `completed`/`failed` 等状态或 file counts，不能由网关伪造即时成功；
- chunking strategy、attributes/filter、expiration 和 ranking/search 参数属于接口能力；
- list/search pagination 不能通过多个 Provider 的 cursor 简单拼接。

第一版若没有持久资源路由，不应对外承诺这一资源族。单 target namespace 仍需 owner isolation、restart behavior 和删除/expiry 处理。

## Responses File Search

Responses 请求中的 `tools: [{"type":"file_search", ...}]` 是 hosted tool。它需要：

- 所选 Responses Native Upstream API 明确支持 file search；
- 每个 `vector_store_id` 可解析到同一 issuing Target/API/credential scope；
- 请求中的 include、filters、max results 和 ranking 参数通过固定接口 capability 预检；
- 输出中的 `file_search_call`、citations、annotations 和可选 included results 按 typed JSON/SSE 原样保留；
- Bridge 默认拒绝，因为 Chat function tool 不能代表 Provider 执行的 hosted retrieval 和 citations。

包含 resource ID 的请求不能跨 issuer fallback。即便多个 candidates 都声称支持 file search，也只有签发这些 vector stores 的 candidate 可执行。

## 安全与观测

- resource ID、filename、attributes、search query、chunks 和 retrieved content 不进入普通日志或 metrics label。
- owner authorization 应覆盖 Upload、Part、File、Vector Store、Batch 和 Responses tool request 的整个链条。
- 限制 upload 总字节/part 数、ingestion 并发、search result 数、included content 和 SSE event size。
- 删除、cancel、complete、attach 等副作用请求不跨 Target fallback；超时以 unknown outcome 处理并允许调用者查询状态。
- 网关不执行本地 embedding、chunking 或 vector search，除非未来另有明确产品需求。

## TDD 与验收矩阵

| 状态机 | 必须覆盖 |
|---|---|
| Upload | expiry、part 顺序/重复、bytes mismatch、complete/cancel 竞争、timeout unknown outcome |
| File mapping | complete 返回 File 后 issuer/owner 继承，重启后仍可 retrieve/delete |
| Vector ingestion | queued/in_progress/completed/failed、poll、batch cancel、partial failure |
| Search | query/filter/ranking/pagination、result limits、resource affinity |
| Responses tool | typed call/result/citation SSE、include results、Bridge 拒绝、target-bound fallback |
| security | forged/cross-user IDs、日志脱敏、body/result limits、credential 与 endpoint 隔离 |

## 非目标

- 不把 `/files` 支持解释为自动支持 Uploads/Vector Stores/File Search；
- 不用本地内存 map 冒充可重启恢复的资源 ledger；
- 不跨 Provider 合并 vector spaces、search score、ranking 或 pagination；
- 不把 hosted file search 降级为普通 function tool；
- 现阶段 1/2 实施范围不包含本协议。
