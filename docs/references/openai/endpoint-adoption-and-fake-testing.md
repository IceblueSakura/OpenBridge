# OpenAI API family 与 fake 合同证据边界

## 来源、范围与快照

本文记录 OpenAI 当前 API Reference 的 endpoint family，以及确定性 fake 对不同 wire/lifecycle 最多能证明什么。本文不记录
OpenBridge 当前实现、产品范围、候选排序或实施清单。

- 官方总入口：[API Overview](https://developers.openai.com/api/reference/overview)；
- 核心补充资料：[Responses WebSocket mode](https://developers.openai.com/api/docs/guides/websocket-mode)、
  [Image generation](https://developers.openai.com/api/docs/guides/image-generation)、
  [Audio and speech](https://developers.openai.com/api/docs/guides/audio)、
  [Realtime](https://developers.openai.com/api/docs/guides/realtime)、
  [Batch](https://developers.openai.com/api/docs/guides/batch)、
  [Video generation](https://developers.openai.com/api/docs/guides/video-generation)；
- 官方资料复核日期：2026-08-10；
- OpenAI 明确允许在 `v1` 中新增 resource、optional parameter、response property 和 stream event type。因此 endpoint、字段、枚举、
  event 与 beta/deprecated 状态必须按本快照理解，真正实施前仍需逐 operation 重新复核。

字段级 request/response 事实由本目录的细粒度 owner 文档维护。本文只保留 endpoint map、聚合适配性、依赖关系和 fake 证据边界，
不复制完整 schema。

## 1. “完整 OpenAI 协议”需要先拆成兼容档位

当前官方目录并不是单一的“模型转发协议”。它同时包含应用数据面、OpenAI 托管资源、异步工作流、双向会话、平台产品、组织管理面和
legacy surface。对多 Provider 聚合网关而言，比较可执行的定义是声明并完成一个或多个兼容档位，而不是承诺镜像官方目录的每个 path。

| 档位 | 范围 | 聚合网关适配性 | 主要所有权 |
|------|------|----------------|------------|
| I：推理数据面 | Chat、Responses、Embeddings、Moderations、Images、Audio | 高 | 每个请求选择 model/route；结果可直接或流式返回 |
| R：资源与状态面 | stored Chat/Responses、Conversations、Files、Uploads、Vector Stores | 中 | opaque id、issuer/account affinity、分页、存储和删除 |
| W：异步工作流 | Batches、Evals、Fine-tuning、Videos、webhook event delivery | 中到低 | 长生命周期 job、状态转换、输出资源和回调 |
| B：Beta 面 | Responses (Multi-agent) 等独立 beta family | 仅按明确 consumer 需求 | beta version/schema、独立 resource/event 与快速演进 |
| P：OpenAI 平台服务 | ChatKit、Containers、Skills、custom voices、content provenance | 低 | OpenAI 托管运行时、产品 identity 或政策能力 |
| A：管理面 | Organization、Projects、Users、Keys、Roles、Usage、Audit 等 | 不属于普通聚合数据面 | 独立 admin credential、租户治理和账单 |
| L：旧版/关闭面 | Completions、Assistants/Threads、Realtime Beta、已宣布关闭的 Videos API | 仅按明确 consumer 需求 | 迁移兼容或限时快照 |

研究推论：若产品目标是“完整的 OpenAI-compatible 多 Provider 聚合转发”，应优先定义 **I 档完整度**，再按真实客户端需求选择 R/W
档；B/P/A/L 不应因为出现在同一 API Reference 中就自动进入默认兼容承诺。

## 2. 端点不仅是 method + path

每个 operation 至少需要固定以下六个维度，才能称为兼容：

1. method、path、query、header、认证与 `Content-Type`；
2. request encoding：JSON、multipart、原始 bytes、SDP 或双向 event；
3. success transport：JSON、SSE、binary stream、WebSocket、WebRTC 或 SIP；
4. error envelope、HTTP status、首个下游 byte 之后的失败与取消语义；
5. model capability、resource issuer/account affinity、opaque id 和状态机；
6. body/media budget、重放安全性、敏感数据与可观测性边界。

因此，“某 Provider 有图片模型”不等于它支持 `/v1/images/generations`；“能上传文件”也不等于 Files、Uploads、Vector Stores 和
Responses `input_file` 共用一种协议。

## 3. Fake 合同测试分层

这里的 fake 指测试进程内或 loopback 上、完全合成且可重复的 upstream/transport/resource simulator。它不能出现在生产模型目录，
也不能被表述为真实 Provider/model 可用。

| 级别 | fake 必须模拟的行为 | 可以证明 | 不能证明 |
|------|---------------------|----------|----------|
| F0 JSON 合同 | request validation、字段 union、成功/错误 JSON、header | router、parser、serializer、错误映射 | 真实模型语义、配额、延迟 |
| F1 媒体 transport | multipart boundary、filename/MIME、binary body、SSE fragmentation/terminal | wire framing、budget、取消、首 byte 边界 | codec 质量、真实媒体有效性、上游计费 |
| F2 资源 lifecycle | opaque id、issuer、CRUD、分页、过期、权限、terminal state | identity/state routing 与 fail-closed 行为 | 真实 retention、跨进程恢复、托管索引 |
| F3 异步 workflow | queued/running/terminal、cancel、output/error resource、幂等与 webhook | job 状态机和回调协议 | 实际排队时长、容量、费用、长期可靠性 |
| F4 双向 session | handshake、typed client/server event、backpressure、close/reconnect | 双向 transport 和 session 状态机 | 真实 WebRTC/SIP 网络、音质、长时负载 |
| P 真实 Provider | 官方 credential 下的真实 model/resource 调用 | 被测账户、model、operation 和样本确实可用 | 未测枚举、格式、负载或其他 Provider |

一个 endpoint 应按其最高风险维度选择最低 fake 级别。例如 Audio Speech 至少需要 F1；Responses resource 至少需要 F2；Batch 至少
需要 F3；Realtime 至少需要 F4。固定返回一个 `200` canned body 只接近 F0，不能替代 lifecycle 或 streaming contract。

## 4. I 档：直接推理与媒体 endpoint

### 4.1 JSON 请求/JSON 结果

| Operation | Method/path | Wire 与状态 | 最低 fake | 聚合判断 |
|-----------|-------------|-------------|-----------|----------|
| Moderations | `POST /v1/moderations` | JSON 文本、文本数组或 text/image part → JSON `results[]` | F0 | 高适配；最小的新 operation contract |
| Embeddings | `POST /v1/embeddings` | JSON → ordered vector list | F0 | 高适配；字段细节见[Embeddings](embeddings-create.md) |
| Images generations | `POST /v1/images/generations` | JSON → URL/Base64；部分 profile 有专用 stream | F0，stream 为 F1 | 高适配；见[Images Generations](images-generations.md) |

Moderations 的 `input` 是 string、string array 或多模态 text/image object array；结果包含实际 model、每项 `flagged`、categories、
category scores 和 category-to-input-type 信息。fake 可以精确验证 union、顺序和 schema，但合成分数绝不证明安全分类质量。字段与动态
model 快照见 [Moderations owner 文档](moderations-create.md)。

### 4.2 Multipart、binary 与专用 stream

| Operation | Method/path | Request → response | 最低 fake | 聚合判断 |
|-----------|-------------|--------------------|-----------|----------|
| Images edits | `POST /v1/images/edits` | multipart image/mask/prompt → URL/Base64 或专用 stream | F1 | 高适配；需独立上传和媒体预算 |
| Images variations | `POST /v1/images/variations` | multipart image → URL/Base64 | F1 | 中；当前主要是 legacy image-model 分支，不能从 edits 推导 |
| Audio speech | `POST /v1/audio/speech` | JSON text/voice/format → binary/streaming audio | F1 | 高适配；是验证 binary response 的清晰切片 |
| Audio transcriptions | `POST /v1/audio/transcriptions` | multipart audio → JSON/text/subtitle 或 transcription stream | F1 | 高适配；同一 path 有多种 response media type |
| Audio translations | `POST /v1/audio/translations` | multipart audio → translated text/JSON | F1 | 高适配；与 transcription 是独立 operation |

图片细节见 [Edits/Variations](images-edits-and-variations.md)；音频细节见
[Speech](audio-speech.md)、[Transcriptions](audio-transcriptions.md)和[Translations](audio-translations.md)。这些 operation 不能复用
Chat data-only SSE 或 Responses typed SSE 的 terminal 规则。

### 4.3 同一模态但不是新 endpoint 的能力

- Chat image/audio/file input 是 `POST /v1/chat/completions` 的 request union 扩展；
- Responses image/audio/file input 是 `POST /v1/responses` 的 item/content union 扩展；
- Responses hosted image generation、file search、web search、code interpreter、computer use 和 MCP 属于 tool item/event contract；
- 它们需要扩展既有 endpoint 的 capability、preflight、request/response 和 event grammar，不能用新增 `/images/*`、`/files/*` 成功来代替。

Endpoint coverage 与 schema/tool coverage 必须分开报告，否则“path 已存在”会掩盖客户端实际无法使用的字段和 event。

## 5. R 档：stored response、conversation 与文件资源

### 5.1 Chat、Responses 与 Conversations

| Resource | Operation map | 最低 fake | 关键依赖 |
|----------|---------------|-----------|----------|
| Stored Chat Completion | `GET /v1/chat/completions`；`GET`/`POST`/`DELETE /v1/chat/completions/{completion_id}`；`GET /v1/chat/completions/{completion_id}/messages` | F2 | create 使用 `store: true`、metadata、cursor、issuer |
| Stored Response | `GET`/`DELETE /v1/responses/{response_id}`；`POST /v1/responses/{response_id}/cancel`；`GET /v1/responses/{response_id}/input_items` | F2 | storage、background status、cancel legality、pagination |
| Responses utility | `POST /v1/responses/input_tokens`；`POST /v1/responses/compact` | F0/F2 | tokenizer/model contract；compaction output ownership |
| Conversation | `POST /v1/conversations`；`GET`/`POST`/`DELETE /v1/conversations/{conversation_id}` | F2 | long-lived identity、metadata、retention |
| Conversation items | `POST`/`GET /v1/conversations/{conversation_id}/items`；`GET`/`DELETE /v1/conversations/{conversation_id}/items/{item_id}` | F2 | ordered items、pagination、item/conversation ownership |
| Responses WebSocket mode | `wss://api.openai.com/v1/responses`，连接内发送 `response.create` event | F4 | persistent connection、incremental input、`previous_response_id`、event terminal；见[owner 文档](responses-websocket.md) |

Stored Chat 详情见 [Stored Chat resources](chat-completions-stored-resources.md)，Responses resource 见
[Resource lifecycle](responses-resource-lifecycle.md)，conversation 语义见 [State ownership](responses-state.md)。这些 endpoint 的 id 必须
回到原 issuer/account；随机重新选 route 会把“资源不存在”与“路由错误”混为一谈。

Responses WebSocket mode 虽复用 `/v1/responses` 和 Responses event vocabulary，但它不是 HTTP SSE 的一个布尔开关。官方当前指南要求持久
WebSocket，每轮发送 `response.create`，并以增量 input 加 `previous_response_id` 继续；`stream` 和 `background` 不是该 event payload 的
transport 字段。

### 5.2 Files、Uploads 与 Vector Stores

| Resource | Operation map | 最低 fake | 聚合判断 |
|----------|---------------|-----------|----------|
| Files | `POST /v1/files`；`GET /v1/files`；`GET`/`DELETE /v1/files/{file_id}`；`GET /v1/files/{file_id}/content` | F1/F2 | 中；multipart、metadata、binary download 和 issuer 缺一不可 |
| Uploads | `POST /v1/uploads`；`POST /v1/uploads/{upload_id}/parts`；`POST .../complete`；`POST .../cancel` | F1/F2 | 中；多请求事务、part 顺序和总量限制 |
| Vector Stores | CRUD/list `/v1/vector_stores` 与 `/v1/vector_stores/{vector_store_id}`；`POST .../search` | F2 | 中；托管索引与检索语义不能由 Files 冒充 |
| Vector Store files | CRUD/list/content `/v1/vector_stores/{vector_store_id}/files/*` | F2 | 中；file identity 与 membership identity 分离 |
| Vector Store file batches | create/retrieve/list/cancel `/v1/vector_stores/{vector_store_id}/file_batches/*` | F2/F3 | 中到低；processing state 与批量 membership |

详细 wire 与 lifecycle 分别见 [Files Create](files-create.md)、[Metadata/Delete](files-metadata-and-delete.md)、
[Content download](files-content-download.md)、[Uploads transaction](files-uploads-transaction.md)和
[Vector Stores](files-vector-stores.md)。Files/Uploads 是多个后续 family 的基础资源，但资源 id 不能跨 Provider 或账户透明互换。

## 6. W 档：异步作业与回调

| Family | 主要 operation | 最低 fake | 采用判断 |
|--------|----------------|-----------|----------|
| [Batches](https://developers.openai.com/api/reference/resources/batches/methods/create) | `POST /v1/batches`；`GET /v1/batches`；`GET /v1/batches/{batch_id}`；`POST .../cancel` | F3 | 有条件；依赖 purpose=`batch` 的 Files、JSONL、output/error files |
| [Evals](https://developers.openai.com/api/reference/resources/evals/methods/create) | Eval CRUD/list；Run create/retrieve/list/delete/cancel；Output Item retrieve/list | F3 | 低；评测定义、data source、grader 与托管执行强耦合 |
| [Fine-tuning](https://developers.openai.com/api/reference/resources/fine_tuning/subresources/jobs/methods/create) | Jobs create/retrieve/list/events/cancel/pause/resume；checkpoints/permissions；alpha graders | F3 | 低；依赖训练 Files、model ownership 和 provider-specific method |
| [Webhooks](https://developers.openai.com/api/reference/resources/webhooks) | OpenAI 向用户 callback URL 投递 response/batch/fine-tune/eval 等 event | F3 | 横向能力；不是普通下游请求 endpoint |
| [Videos](https://developers.openai.com/api/docs/guides/video-generation) | create/lifecycle/download/derived jobs/characters | F3 | 当前不宜新接；官方已宣布 2026-09-24 关闭现有 Sora 2 Videos API |

官方当前 Batch create 接受 `/v1/responses`、`/v1/chat/completions`、`/v1/embeddings`、legacy `/v1/completions`、
`/v1/moderations`、`/v1/images/generations`、`/v1/images/edits` 与 `/v1/videos`。这意味着 Batch 不是一个简单的通用转发 path：它还要
验证每行 endpoint、保持 `custom_id`、生成 output/error file、处理取消与 terminal status，并把每条请求固定到可执行 issuer。

Webhook reference 描述的是官方服务发出的 HTTP request。聚合网关若承诺兼容，需要定义 callback URL 信任、签名验证/重新签名、event
identity 重写、重投、乱序和重复投递；仅能解析 webhook event schema 不等于已经提供 webhook delivery。

Videos 的关闭日期是强时效边界。现有 owner 文档只保留关闭前协议快照，见[视频索引](README.md#8-视频)；替代 API 公布前不应把它作为
新的长期核心端点。

## 7. Realtime：HTTP control plane + 双向 data plane

| 面 | 当前入口 | 最低 fake | 关键事实 |
|----|----------|-----------|----------|
| Client secrets | `POST /v1/realtime/client_secrets`；`POST /v1/realtime/translations/client_secrets` | F2 | 短期 credential、scope、TTL，不是普通 API key |
| WebRTC calls | `POST /v1/realtime/calls`；`POST /v1/realtime/translations/calls` | F4 | multipart/SDP signaling 与 media/data channel |
| SIP call control | `POST /v1/realtime/calls/{call_id}/accept`、`reject`、`hangup`、`refer` | F4 | incoming-call identity 与电话状态机 |
| WebSocket data plane | `/v1/realtime`；`/v1/realtime/translations` | F4 | 双向 typed JSON/audio event、backpressure、close/reconnect |
| Legacy Realtime Beta | `POST /v1/realtime/sessions`；`POST /v1/realtime/transcription_sessions` | F2/F4 | 只为明确 legacy client 固定旧 schema |

完整 endpoint 与 transport 边界见 [Realtime control plane](realtime-control-plane.md)和
[Realtime transport](realtime-transport.md)。Audio Speech/Transcription 的 F1 成功不能外推为 Realtime；Responses SSE 或 Responses
WebSocket 的 event vocabulary 也不能外推为 Realtime session。

## 8. B/P/A/L 档：不应默认纳入普通聚合数据面

### 8.1 Beta Responses (Multi-agent)

当前 API Overview 另列 [Responses (Multi-agent) beta](https://developers.openai.com/api/reference/resources/beta/subresources/responses/methods/create)，
并为其列出 create/retrieve/delete/cancel/compact、input items、input tokens、streaming events 与 WebSocket events。它与稳定 Responses
目录平行出现，不能仅凭 operation 名称相同就推断 path、header、request union、agent identity、event 或 resource lifecycle 等价。

在有明确 beta consumer 前，不应把稳定 `/v1/responses` 的实现声明为 Multi-agent beta 兼容；采用时也应独立固定官方 beta 版本、SDK、
required header（如有）、wire fixture 与失效/迁移策略。fake 至少覆盖 F2/F4，但不能证明 beta 服务可用或未来兼容。

### 8.2 OpenAI 平台专属资源

| Family | 官方 surface | 为什么不是普通模型转发 |
|--------|--------------|------------------------|
| [ChatKit](https://developers.openai.com/api/reference/resources/beta/subresources/chatkit/subresources/sessions/methods/create) | sessions create/cancel；threads retrieve/delete/list/items | 绑定 workflow、end-user identity、client secret 和 OpenAI 托管 ChatKit |
| [Containers](https://developers.openai.com/api/reference/resources/containers/methods/create) | container CRUD/list；container files CRUD/list/content | 绑定托管执行环境、network policy、secret、expiry 与文件系统 |
| [Skills](https://developers.openai.com/api/reference/resources/skills/methods/create) | skill CRUD/list/content；version CRUD/list/content | 绑定 OpenAI hosted tool/runtime 与版本化 artifact |
| [Custom voices](https://developers.openai.com/api/reference/resources/audio/subresources/voice_consents/methods/create) | voice consent CRUD/list；voice create | 绑定 consent、声音样本、账户政策和 voice identity |
| [Content provenance checks](https://developers.openai.com/api/reference/resources/content_provenance_checks/methods/create) | `POST /v1/content_provenance_checks` multipart → JSON | OpenAI 专用 provenance detector，不是通用 AI/media 检测标准 |

这些 family 可以按某个 Provider 的专属扩展实现，但不应伪装为所有 Provider 都可执行的通用能力。fake 最多验证 F1/F2 contract；真实平台
identity、政策和执行能力仍必须逐 Provider 验证。

### 8.3 Models 与 Administration

- `GET /v1/models` 和 `GET /v1/models/{model}` 是客户端发现面；聚合网关通常应返回自身可执行的公共模型，而不是复制某个上游目录；
- [`DELETE /v1/models/{model}`](https://developers.openai.com/api/reference/resources/models/methods/delete) 在官方语义中用于删除
  fine-tuned model，并不是“从网关目录隐藏一个公共模型”的对称操作；
- Administration 包含 admin API keys、audit logs、certificates、data retention、groups、roles、users、invites、projects、service accounts、
  permissions、rate/spend limits 和 usage；官方要求独立 Admin API key；
- 因此 Administration 应保持独立认证域与租户所有权。普通用户 Bearer data-plane gateway 不应透明代理上游组织管理员操作。

### 8.4 Legacy

Legacy Completions、Assistants/Threads/Runs 和 Realtime Beta 只有在明确 consumer 仍发送这些协议时才值得形成兼容 profile。为追求 path
数量而新建 legacy facade 会引入另一套 state、stream 和 tool lifecycle，却不能提高现代 Responses 客户端兼容度。

## 9. 证据边界

- endpoint map 只证明固定日期的官方 surface，不表示 OpenBridge 或任何 Provider 已实现对应 operation；
- fake 只证明其覆盖级别内的 wire、budget 或 state machine，不证明真实模型质量、费用、配额、账户权限或长期稳定性；
- Provider 标注 “OpenAI-compatible” 不能替代逐 operation 的 method/path、encoding、stream grammar、错误、模型和限制证据；
- endpoint、schema、transport、lifecycle、capability 与 evidence 必须分开记录，不能由 path 数量或 fake `200` 数量代替。
