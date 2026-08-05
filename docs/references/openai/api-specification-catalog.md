# OpenAI API 现有规范清单

## 目的、范围与快照

本文是学习和实现协议转换前的 API 地图，不是对每个字段的重复抄录。它以 OpenAI 官方 OpenAPI 规范和 API Reference 为准；字段支持仍取决于具体 model、账户权限和 beta 状态。

- **初次采集日期**：2026-07-18；**扩展 endpoint 复核日期**：2026-08-04。
- **机器可读规范**：官方 endpoint catalog，OpenAPI `3.1.0`，`info.version=2.3.0`；2026-08-04 读取到 **182 个 endpoint path**。该在线目录会变化，数字只标识本次快照。
- **人读规范**：OpenAI Developers API Reference 及官方 guides。
- **本系列的重点**：`/chat/completions` 与 `/responses`；其完整学习文档见 [Chat Completions 协议](chat-completions-protocol.md) 和 [Responses 协议](responses-protocol.md)。
- **本系列的扩展专题**：总览见 [Embeddings 与多模态 API 关系调研](embedding-and-multimodal-forwarding.md)，逐协议资料见[扩展协议调研索引](protocol-details/README.md)。

官方来源：

1. https://github.com/openai/openai-openapi
2. https://raw.githubusercontent.com/openai/openai-openapi/master/openapi.json
3. https://developers.openai.com/api/reference
4. https://platform.openai.com/docs/guides/migrate-to-responses

## 1. 阅读规范的层次

| 层 | 应作为什么 | 适合解决的问题 |
|---|---|---|
| OpenAPI 3.1 | wire contract 的机器可读快照 | path、HTTP method、参数类型、response schema、生成客户端/contract tests |
| API Reference | 当前字段语义的权威说明 | 必填/可选、枚举值、对象生命周期、模型限制 |
| Guides | 跨请求行为和实践 | 流、工具循环、状态、structured output、background、费用与安全 |
| Model docs / changelog | capability matrix 与变更记录 | 某 model 是否支持音频、reasoning、工具、参数或 beta feature |

实现 proxy 时，不能只按 OpenAPI generator 的类型作判断：工具循环、`previous_response_id`、SSE 事件先后、store/TTL 和 model-specific 参数约束属于运行语义，必须从 Reference/Guides 同时读取。

## 2. 端点族总览

下表按资源语义归类。列出的 path 是规范中最值得先学习的主路径，`{id}` 表示对应资源的 retrieve/update/delete 或子资源；它不是 179 个 path 的逐条抄录。

| 端点族 | 主路径 | 用途与学习优先级 |
|---|---|---|
| **Responses** | `/responses`, `/responses/{response_id}`, `/responses/{response_id}/input_items`, `/responses/{response_id}/cancel`, `/responses/compact`, `/responses/input_tokens` | 当前 agent/text/tool 主 API；有 item graph、状态、SSE、background 与 server-side state。**最高** |
| **Chat Completions** | `/chat/completions`, `/chat/completions/{completion_id}`, `/chat/completions/{completion_id}/messages` | 兼容性最广的 message/choice API；仍受支持，新项目官方建议优先考察 Responses。**最高** |
| **Conversations** | `/conversations`, `/conversations/{conversation_id}`, `/conversations/{conversation_id}/items` | Responses 的持久 conversation/item 容器，区别于单次 `previous_response_id` 链。**高** |
| **Realtime** | `/realtime/sessions`, `/realtime/client_secrets`, `/realtime/transcription_sessions`, `/realtime/calls/*` | 实时音频/事件会话；HTTP 创建 session/secret，媒体与事件经 WebSocket/WebRTC/SIP。**高（仅需实时场景）** |
| **Embeddings** | `/embeddings` | 文本向量；请求/响应相对独立、无 tool loop。**中** |
| **Images / Video** | `/images/generations`, `/images/edits`, `/images/variations`; `/videos`, `/videos/{video_id}`, `/videos/edits`, `/videos/extensions`, `/videos/remix`, `/videos/characters/*` | 专用媒体生成资源；视频为可轮询 resource。**中** |
| **Audio** | `/audio/speech`, `/audio/transcriptions`, `/audio/translations`, `/audio/voices`, `/audio/voice_consents/*` | TTS、STT、翻译与 voice consent/resource。**中** |
| **Moderation** | `/moderations` | 独立安全判定；也可与 generation moderation 特性关联。**中** |
| **Models** | `/models`, `/models/{model}` | 模型枚举/元数据；不等同于完整 capability/定价来源。**中** |
| **Files / Uploads** | `/files`, `/files/{file_id}/content`; `/uploads`, `/uploads/{upload_id}/parts`, `/uploads/{upload_id}/complete` | 上传与读取文件；是 file input、batch、fine-tuning、vector store 的基础。**中** |
| **Vector stores / retrieval** | `/vector_stores`, `/vector_stores/{id}/files`, `/vector_stores/{id}/file_batches`, `/vector_stores/{id}/search` | 托管检索数据面；与 Responses file search 相关但不是同一个 request schema。**中** |
| **Containers / Skills** | `/containers`, `/containers/{id}/files/*`; `/skills`, `/skills/{id}/content`, `/skills/{id}/versions/*` | code execution/container 文件和可版本化 skill 资源。**按需** |
| **Batch** | `/batches`, `/batches/{batch_id}`, `/batches/{batch_id}/cancel` | 离线批处理，输入通常是 JSONL 中的普通 API 请求。**按需** |
| **Fine-tuning / graders** | `/fine_tuning/jobs/*`, `/fine_tuning/checkpoints/*`, `/fine_tuning/alpha/graders/*` | 训练 job、checkpoint 权限、grader 运行/校验。**按需** |
| **Evals** | `/evals`, `/evals/{id}/runs`, `/evals/{id}/runs/{run_id}/output_items` | 评估定义与运行结果。**按需** |
| **ChatKit** | `/chatkit/sessions`, `/chatkit/threads`, `/chatkit/threads/{id}/items` | UI/agent conversation product面；不要与 `/conversations` 误混。**按需** |
| **组织/项目管理** | `/organization/*`, `/projects/*` | users、roles、groups、API keys、certificates、audit logs、usage、costs、rate limits、retention、spend alerts。**管理面** |
| **Assistants / Threads** | `/assistants`, `/threads`, `/threads/{id}/messages`, `/threads/{id}/runs/*` | 仍在 OpenAPI 内，但官方已于 2025-08-26 deprecate，公告 sunset 为 2026-08-26；新实现不应以它作为主协议。**只做迁移兼容** |
| **Legacy Completions** | `/completions` | prompt string 时代的补全 API；只在保留兼容需要时实现。**低** |

## 3. 影响转换器边界的 API 分类

### 3.1 一次请求完成型

典型：embeddings、moderations、audio、images，以及非 stream 的 Chat Completions。转换器主要关注：字段合法性、multipart/JSON body、二进制/JSON response、usage 和错误映射。

### 3.2 事件型

典型：Chat Completions `stream=true`、Responses `stream=true`、Realtime。它们都产生增量，但协议不同：

- Chat 为 data-only SSE chunk，核心增量在 `choices[].delta`。
- Responses 为带 `type` 的语义 SSE event，例如 `response.created`、`response.output_text.delta`、`response.completed`。
- Realtime 是双向 session event stream，不应套用 HTTP SSE 的状态机。

官方 streaming guide：https://platform.openai.com/docs/guides/streaming-responses

### 3.3 资源生命周期型

典型：Responses、conversations、files/uploads、vector stores、videos、batches、fine-tuning、evals、organization/project resources。这类 API 不能只实现 `POST create`：如果对外承诺兼容，还应决定 retrieve/list/cancel/delete、权限、TTL、polling 和 restart recovery 的边界。

### 3.4 工具/agent orchestration 型

Responses、Chat Completions、Realtime 均可参与工具调用；但 Responses 有 built-in tool items、function-call output item、conversation/background/compaction 等更丰富的状态面。需要保存 correlation id、item 顺序、tool result 和 stream completion，而不只是拼接文本。

## 4. 推荐学习顺序

1. 先读 [Chat Completions 协议](chat-completions-protocol.md)，掌握 message、`choices[]`、tool-call round trip 与 chunk stream。
2. 再读 [Responses 协议](responses-protocol.md)，掌握异构 `input[]`/`output[]`、typed events、response state 与 continuation。
3. 对照官方迁移指南，明确哪些结构可映射、哪些会有信息损失：
   https://platform.openai.com/docs/guides/migrate-to-responses
4. 最后按实际产品范围选择 Realtime、Conversations、Files/Vector Stores、Batch 或管理面；不要因 OpenAPI 中存在就默认实现。

## 5. 规范使用注意事项

- **模型能力优先**：Reference 明确提示参数支持因模型而异，尤其 reasoning models；schema 接受不等于目标 model 一定接受。
- **beta path 单独对待**：OpenAPI 中有 `?beta=true` 的 Responses path。它们不能和稳定 path 共用“完全等价”的 proxy 路由声明。
- **不要把 API docs 与 SDK surface 混为一体**：SDK 的 `responses.create()` 是对 `POST /responses` 的语言封装，wire compatibility 应以 HTTP/JSON/SSE contract 为基准。
- **保存来源版本**：后续做实现时，建议把本次 OpenAPI snapshot 的 commit SHA、下载日期和 diff 结果纳入 CI；官方 master 会变化。
