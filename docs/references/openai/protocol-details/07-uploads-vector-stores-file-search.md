# OpenAI Uploads、Vector Stores 与 File Search 协议调研

## 1. 与 Files 的关系

Uploads、Vector Stores 和 Responses File Search 组成多个资源层：Upload 负责分片事务，File 是完成后的对象，Vector Store
管理检索集合，File Search 是 Responses hosted tool。

资料：[Uploads API](https://developers.openai.com/api/reference/resources/uploads)、[Vector Stores API](https://developers.openai.com/api/reference/resources/vector_stores)、[File search](https://developers.openai.com/api/docs/guides/tools-file-search)。

## 2. Upload transaction

- create upload 后得到临时 upload id 与 expiry；
- parts 可分批上传并拥有各自 identity/order；
- complete 将 parts 组装为最终 file；
- cancel/expiry 结束未完成事务。

2026 年既有资料曾描述约 8 GB 与约一小时 expiry；这些是当日 service profile，使用前必须重新核对。

## 3. Vector Store lifecycle

Vector Store 是独立资源，可关联 files、形成 processing status，并支持 list/retrieve/update/delete 等操作。file processing
完成前不能假定检索可用。

## 4. Responses File Search

File Search 通过 hosted tool 引用 vector-store identity。tool request、检索结果 include、citation/annotation 和资源权限共同决定
response shape。

## 5. 边界

- upload/file/vector-store/tool-call identity 各自独立。
- resource ownership、TTL、processing failure 与 delete 不能由一次 Responses request 隐式替代。
- hosted retrieval 可能产生额外存储、索引、搜索和数据保留成本。
- 对这些 resource method 的 retry 必须考虑事务状态与副作用。

