# Embedding 与多模态 API 转发参考

## 目的与证据边界

本文补充 OpenAI API 参考框架中与 embedding、文件、图像、音频/语音以及专用媒体 endpoint 相关的转发事实。它服务于后续 OpenBridge API 聚合实现的协议建模、路由约束和测试设计，不构成当前服务已经支持这些 endpoint 的承诺。逐协议细节见[扩展协议实现细节索引](implementation-details/README.md)；现阶段只有编号 1 Embeddings 进入类型化能力与 Native 转发焦点，编号 2 多模态仅保留为已批准需求，见[功能需求](../../functional-requirements/embedding-and-native-multimodal.md)和[当前开发焦点](../../implementation-plans/current-focus.md)。

本文区分三类内容：

- **官方协议事实**：来自 OpenAI API Reference、官方 guide 和官方 OpenAPI endpoint schema。
- **当前 OpenBridge 事实**：来自本 checkout 的源码、OpenAPI 和已存在的能力/路由边界。
- **实现规则推论**：根据前两类事实为网关提出的最小安全约束；实施前仍需转成需求、失败测试和实际 Provider 证据。

**复核时间：** 2026-08-04（Asia/Shanghai）。本次官方 endpoint schema 返回 `OpenAPI 3.1.0`、`info.version=2.3.0`；官方文档和 model capability 会继续变化，文中的限制值不能替代实施前的再次核对。

官方入口：

- [OpenAI API Reference](https://developers.openai.com/api/reference)
- [官方 OpenAPI endpoint 列表](https://developers.openai.com/api/reference)
- [openai/openai-openapi](https://github.com/openai/openai-openapi)
- [Images and vision](https://developers.openai.com/api/docs/guides/images-vision)
- [File inputs](https://developers.openai.com/api/docs/guides/file-inputs)
- [Audio and speech](https://developers.openai.com/api/docs/guides/audio)
- [File transcription](https://developers.openai.com/api/docs/guides/speech-to-text)
- [Text to speech](https://developers.openai.com/api/docs/guides/text-to-speech)
- [Migrate to the Responses API](https://developers.openai.com/api/docs/guides/migrate-to-responses)

## 1. 端点族地图

这些 endpoint 不能都套用当前 Chat/Responses 的同一个 JSON/SSE 转发器。请求体、响应体、流生命周期、资源状态和重试安全性至少有以下差异：

| API 族 | OpenAI 路径 | 请求形状 | 成功响应 | 流 | 主要状态边界 |
|---|---|---|---|---|---|
| Embeddings | `POST /v1/embeddings` | `application/json` | JSON 向量列表 | 否 | 无会话；输出向量的模型/维度/编码必须稳定 |
| Files | `GET/POST /v1/files`、`GET/DELETE /v1/files/{file_id}`、`GET /v1/files/{file_id}/content` | 查询、`multipart/form-data`、二进制下载 | 文件资源 JSON 或原始字节 | 通常否 | `file_id` 由 Provider/项目作用域签发 |
| Uploads | `POST /v1/uploads`、`/parts`、`/complete`、`/cancel` | JSON 创建/完成，分片 part 上传 | Upload 状态或最终 File 资源 | 通常否 | 中间 Upload 有过期、分片顺序和一次性完成状态 |
| Speech-to-text | `POST /v1/audio/transcriptions` | `multipart/form-data` | JSON、文本/字幕或 SSE transcript event | 可选 | 上传文件可重放，但重复计算和超时后的已接受请求需单独处理 |
| Speech translation | `POST /v1/audio/translations` | `multipart/form-data` | JSON、文本/字幕 | 当前参考 schema 无 SSE 成功响应 | 模型和输出格式受 endpoint/model 限制 |
| Text-to-speech | `POST /v1/audio/speech` | `application/json` | 原始二进制（当前 schema 为 `application/octet-stream`）或 SSE audio event | 可选 | 原始音频以字节 EOF 结束；SSE 模式另有 terminal 规则 |
| Chat 多模态 | `POST /v1/chat/completions` | JSON message content parts | Chat JSON 或 Chat SSE | 可选 | 输入 part 顺序、audio/file/image 形状和输出 audio 不能丢失 |
| Responses 多模态 | `POST /v1/responses` | JSON input items/content parts | Responses JSON 或 typed SSE | 可选 | item、resource ID、continuation 和 hosted tool 具有 issuer/state 语义 |
| Images | `POST /v1/images/generations`、`/edits`、`/variations` | generation 为 JSON；当前 edit schema 以 JSON reference 为主；variation 为 multipart | JSON 中的 URL/base64，generation/edit schema 也列出 SSE | 部分支持 | guide/SDK 与不同模型的文件输入会演进，实施时须按目标 profile 复核 |
| Videos | `POST /v1/videos`、`/edits`、`/{video_id}`、`/content` 等 | JSON 或 multipart | 异步资源 JSON 或原始视频字节 | 轮询资源状态 | 创建、轮询、取消/编辑和内容下载是独立的长任务生命周期 |
| Realtime voice | `/v1/realtime/*` | WebSocket/WebRTC/SIP 或 session HTTP | 双向事件/媒体 | 双向 | 不是 HTTP JSON/SSE；需要独立 session、媒体格式和取消模型 |

`/v1/audio/voices`、`/v1/audio/voice_consents/*`、Uploads、Vector Stores、Batch 和 Containers 是资源/辅助 API，应分别决定是否纳入公共兼容面，不能因为它们存在于官方 OpenAPI 就自动加入当前网关。

## 2. Embeddings：不是生成 API 的一个布尔字段

### 2.1 官方请求/响应事实

官方 `POST /v1/embeddings` 当前接受 JSON 请求，核心字段包括：

| 字段 | 当前官方语义 | 转发注意事项 |
|---|---|---|
| `model` | 要使用的 embedding model | 下游 Public Model 与上游 model 的映射仍应由受信 registry 控制 |
| `input` | 单个字符串、字符串数组、token 数组或 token 数组的数组 | 保留输入顺序；不要把 token 数组误当作普通文本再编码 |
| `encoding_format` | `float` 或 `base64` | 这是响应编码，不是把向量改成字符串的网关本地选项 |
| `dimensions` | 仅部分较新的 embedding model 支持的输出维度约束 | 不应由网关截断或补零；必须按上游 model contract gate |
| `user` | 用于上游滥用检测/终端用户标识的可选值 | 需要独立的隐私和下游 identity policy，不能把 credential 或内部 user id 直接透传 |

当前官方 reference 对 embedding 输入给出单输入 token 上限、单请求总 token 上限以及输入数组大小限制；本次检索显示的值为每输入 8192 tokens、单请求合计 300,000 tokens、数组不超过 2048 项。实现时应把这些视为当前 OpenAI profile 的验证依据，而不是所有 Provider 的通用上限。

成功响应的稳定形状是 `object: "list"`、有序的 `data[]`、每项的 `embedding` 与 `index`、响应 `model` 和 `usage`。`float` 响应可能很大，`base64` 响应又有自己的解码边界；两者都不应经过通用 JSON 文本归一化器重排或舍入。OpenBridge 需要把响应 `model` 投影回下游 Public Model，避免真实 upstream model 泄漏，同时保持向量、index、object 和 usage 的值语义。

官方参考：[Create embeddings](https://developers.openai.com/api/reference/resources/embeddings/methods/create)、[Vector embeddings guide](https://developers.openai.com/api/docs/guides/embeddings)。

### 2.2 聚合网关的语义约束

embedding 的网络失败可以像普通 JSON 请求一样在响应提交前重试，但**向量语义不是普通生成文本语义**：不同 Provider、model、版本、维度或归一化约定产生的向量不能默认混入同一个向量索引。

建议把 embedding 的兼容身份至少视为以下元组：

```text
embedding model family/version
+ output dimensions
+ encoding format
+ normalization/metric contract
+ tokenizer or input encoding contract
```

因此：

- 同一 Provider/Target 的 credential retry 可以沿用通用的输出前 retry 边界，但必须有独立的 body replay 预算检查；超过 replay budget 的合法请求只执行第一次 attempt，不能无界缓存，也不能因内部重放优化额外拒绝。
- 跨 Provider fallback 只有在 registry 明确声明向量身份等价、维度一致且下游接受该等价性时才可启用；默认应关闭或在预检阶段拒绝。
- 不要把 Chat/Responses 的 Bridge 用于“把文本请求转换成 embedding 请求”，也不要把 embedding response 当作 Chat message。
- `dimensions`、`encoding_format`、批量输入和 `usage` 都属于 endpoint contract；不应由一个面向生成模型的 `GenerationCapabilities` 布尔字段代替。

### 2.3 可执行能力而不是模型标签

扩展 Models 接口应新增独立 `interfaces.embeddings`，至少公开 input forms、默认/可显式请求的 output encoding、默认维度、可请求维度域、有效批量/token 上界与顶层 `supported_parameters`。省略字段时的 encoding/dimension default 与“请求能否显式携带该字段”是不同事实；allowed domain 需要 `null`、集合、区间或离散集合，不能只用布尔值。

多 Route 聚合时，集合求交、数值上限取小、dimension domain 求交；默认维度或内部 vector identity 不一致时不能形成一个可 fallback interface。vector identity 属于私有 registry 事实，只做等价判断，不应通过 Models API 暴露 Provider/upstream model 拓扑。当前首版实现限制为单条 Native Embeddings Route。

## 3. 多模态输入的 wire 形状

### 3.1 Chat 与 Responses 的对应关系

Chat 和 Responses 都能承载多模态输入，但它们不是同一个 content schema：

| 逻辑内容 | Chat Completions | Responses | 关键风险 |
|---|---|---|---|
| 文本 | message `content` 中的 `text` part，或纯字符串 | `input_text`，或顶层字符串 | 不要只保留拼接后的文本而丢掉原 part 顺序 |
| 图像 | user-message `image_url.url` 使用远程 URL/data URL，detail 位于嵌套对象 | `input_image` 的 `image_url` 使用远程 URL/data URL，或使用 `file_id`；detail 位于 part 顶层 | URL、data URL、file ID 的 issuer 和大小边界不同 |
| 文件 | user-message `file.file_data`（官方描述为 base64 string）或 `file.file_id`，可带 filename；没有标准 MIME/`file_url`/detail | `input_file` 的 `file_data`（guide 使用 data URL）、`file_url` 或 `file_id`，可带 filename/detail | inline encoding 不同；文件引用不是跨 Provider 可移植的普通字符串，source 必须一选一 |
| 音频输入 | user-message `input_audio.data` 为 base64，当前标准 format 为 `wav`/`mp3` | 本阶段不从 Chat 字段推断 Responses audio part | Chat audio、文件转写和 Realtime voice 是三种不同语义 |
| 音频输出 | `modalities: ["text", "audio"]` 加 `audio` 配置 | Responses 是否开放相同 output audio item 取决于当前 model/API profile | 不能把音频 output data 静默降成 transcript 文本 |

官方 Chat reference 明确说明 message 可能包含 text、image、file 和 audio 等 content part；官方 Responses reference 的当前示例覆盖 `input_image` 和 `input_file`。模型能力仍然决定字段是否可用，不能由 schema 接受就向所有 Public Model 宣称支持。

当前官方 Images guide 把 `original` 列为 Chat/Responses 图像 detail，但 2026-08-04 的官方 Python SDK 生成类型中，Responses 已包含 `original`，Chat 仍只列 `auto/low/high`。这种官方 artifact 漂移正是 detail 必须成为逐 Upstream API 集合的原因；实施时要以目标 schema、SDK 和真实 endpoint 的共同证据决定，而不是维护一个全局枚举。

参考：[Chat API](https://developers.openai.com/api/reference/resources/chat)、[Create a model response](https://developers.openai.com/api/reference/resources/responses/methods/create)、[Images and vision](https://developers.openai.com/api/docs/guides/images-vision)。

### 3.2 图像输入

图像输入常见来源是远程 URL、data URL/base64 或已经由上游签发的 file reference。三者的转发策略不同：

- 远程 URL 是业务 payload，不是 OpenBridge 的 upstream endpoint；但上游可能会主动抓取它，必须明确是否允许任意外部 URL、是否采用域名 allowlist、是否允许重定向以及如何限制大小和下载时间。
- data URL 会把媒体内容直接放入 JSON，不能在日志、错误上下文、metrics label 或 debug output 中记录；解析时要有 JSON body、data URL 和解码后大小上限。
- file reference 只能在其签发的 Provider/Target/项目作用域内解释。跨 Target fallback 可能得到“文件不存在”或更危险的同名/错误资源，不应静默尝试。

图像输入只表达模型消费图像，不等同于 `/v1/images/generations` 的图像生成，也不等同于 File API 的持久资源管理。三个 API 族必须有独立的 capability 和 response contract。

### 3.3 文件输入与文件资源

官方 File API 的创建操作使用 `multipart/form-data`，请求包含二进制 file object 和 `purpose`。当前官方 reference 列出 `assistants`、`batch`、`fine-tune`、`vision`、`user_data`、`evals` 等 purpose；不同 purpose 的格式、生命周期和可用 endpoint 不相同。官方 schema 当前还报告单文件 512 MB、项目总存储 2.5 TB 等服务限制，这些是上游服务限制，不应直接当成 OpenBridge 默认 body limit。

File API 至少包括以下不同的操作语义：

1. upload：产生一个由上游签发的 `file_id`，属于有副作用的资源创建。
2. list/retrieve：读取上游资源元数据，必须保留分页/排序/过滤参数的语义。
3. delete：资源删除，不能因为另一个 Target 没有该 ID 就自动重试到那里。
4. content：下载原始字节，响应不能经过 JSON parser 或 SSE decoder。

对于大文件，官方还提供 Uploads：先创建中间 Upload，再上传 Parts，最后 complete 生成普通 File；当前 Create Upload reference 报告总容量上限为 8 GB、Upload 创建后约一小时过期。它不是 `/files` 的另一种 Content-Type，而是带有分片状态、过期和完成副作用的独立资源。网关不能把 part upload、complete 和普通 File upload 共用无状态 retry/fallback；至少要保存 Upload issuer、part identity、完成状态和取消边界。

文件上传的安全和一致性要求高于普通 JSON：

- 若直接转发原始 multipart bytes，必须同时保留原始 `Content-Type` 的 boundary；不能把它改写成没有 boundary 的 `multipart/form-data`。
- 若网关解析后重建 multipart，必须使用受信的字段 allowlist、文件大小/part 数量限制和规范化的 filename 处理；不能把任意下游 header 或 multipart part 当成 Provider 控制面输入。
- upload 超时可能发生在上游已经接受文件之后。没有明确的 idempotency 或 upload 状态查询契约时，不应使用普通 retry/fallback 自动重放有副作用的 upload。
- 返回的 `file_id` 必须记录 issuer/Provider/Target/credential scope，或者转换为网关自己的 opaque ID；当前 OpenBridge 没有通用 resource ledger，因此不能假设跨 Route 可恢复。

参考：[Create file](https://developers.openai.com/api/reference/resources/files/methods/create)、[Create upload](https://developers.openai.com/api/reference/resources/uploads/methods/create)、[File search guide](https://developers.openai.com/api/docs/guides/tools-file-search)。

### 3.4 source-aware Public Model contract

现阶段 image/file/audio input 需要三个协议内子契约，而不是三个 bool：

| 子契约 | 必须相交的集合 | 必须取小的上限 |
|---|---|---|
| image | remote/data/file-ID source、inline media type、detail default/allowed domain | part 数、remote URL 长度、单 part inline 编码/解码字节 |
| file | inline/remote/file-ID source、raw-base64/data-URL encoding、可验证 media type、detail default/allowed domain 及适用文件类别 | part 数、remote URL 长度、单 part inline 编码/解码字节 |
| audio | inline source、audio format | part 数、单 part inline 编码/解码字节 |
| interface total | 可用子契约 | 总媒体 part、累计 inline 编码字节、累计 inline 解码字节 |

Public Model 的 `modalities.input` 仍只是模型/接口摘要；客户端能否发送某个具体 part 必须由 `multimodal_input` 子契约回答。内容 part 内的 source/detail/format 不加入顶层 `supported_parameters`。任何 Bridged candidate 在本阶段都对媒体集合贡献空集，保证 Models API、preflight 和固定候选不会出现“公开支持但 fallback 无法执行”的矛盾。

## 4. Audio/voice 转发

### 4.1 请求型音频 API

| Endpoint | 请求 | 响应 | 当前官方/实现注意事项 |
|---|---|---|---|
| `/v1/audio/transcriptions` | multipart file、model 以及 response format/language/prompt 等字段 | JSON、字幕/文本格式或 SSE transcript event | 当前 reference 支持多种音频文件格式和可选 streaming；模型对 diarization、timestamps、logprobs 的支持不同 |
| `/v1/audio/translations` | multipart file、model、prompt、response format 等 | JSON 或文本/字幕格式 | 当前 reference 将 `whisper-1` 作为可用模型并未列出与 transcription 相同的 streaming response；不能自动套用 transcription schema |
| `/v1/audio/speech` | JSON `model`、`input`、`voice`、`response_format` 等 | 原始音频二进制，或 `stream_format: "sse"` 的 SSE audio event | 当前 endpoint schema 以 `application/octet-stream` 表示 raw 成功体；实际音频格式由请求/profile决定 |

转发层必须按响应 media type 分流：

- transcription 的 SSE 是结构化 transcript event，使用 SSE framing、事件大小、终态和取消规则。
- speech 的默认成功体是音频字节，EOF 是传输结束，不存在 `[DONE]` 或 `response.completed` 这样的通用 Responses terminal。
- speech 的 `stream_format: "sse"` 才进入 SSE parser；不能把所有 `audio/*` body 当 SSE，也不能把 SSE data 中的 base64 当成最终 raw audio 而不按事件语义组装。
- 二进制响应的 `Content-Type`、可选 `Content-Disposition`、长度/EOF 和上游错误必须在安全 response-header 规则下处理；不能把上游 credential、内部 endpoint 或任意 hop-by-hop header 带回下游。

官方参考：[Create transcription](https://developers.openai.com/api/reference/resources/audio/subresources/transcriptions/methods/create)、[Create translation](https://developers.openai.com/api/reference/resources/audio/subresources/translations/methods/create)、[Create speech](https://developers.openai.com/api/reference/resources/audio/subresources/speech/methods/create)、[Speech-to-text guide](https://developers.openai.com/api/docs/guides/speech-to-text)、[Text-to-speech guide](https://developers.openai.com/api/docs/guides/text-to-speech)。

### 4.2 Voice identity 与 Realtime 的边界

`voice` 既可能是官方内置 voice 名称，也可能是 Provider-specific/custom voice ID；它不是模型 ID，也不是可以在 fallback 时任意迁移的普通参数。若一个 Route 返回了 custom voice 或 voice consent 资源，后续请求必须保持同一 issuer/Target，除非网关明确实现了资源映射和权限验证。

Realtime voice 另有 session、双向事件、音频编码、VAD、WebSocket/WebRTC/SIP 和长连接取消语义。当前 OpenBridge 的 HTTP/SSE transport、`ApiProtocol` 和 upstream response lifecycle 不能直接证明 Realtime 兼容。`/v1/audio/speech` 的一次性 TTS 和 Realtime speech-to-speech 应保持独立的 transport/capability 类型。

## 5. Images API 与多模态输入的分离

图像生成/编辑/变体 endpoint 的返回通常是 JSON 中的 base64 或 URL；当前官方 generation schema 也列出了部分 SSE generation event。它们与 Chat/Responses 的 `image_url`/`input_image` 不同：前者是媒体产物，后者是模型输入。

Video endpoint 还要额外处理异步 resource status、轮询间隔、取消/编辑和最终 content 下载；不能把一次视频创建包装成普通文本生成，也不能把轮询失败当成可安全切换 Target 的普通 5xx。

| 形状 | 主要 payload | 不能做的隐式转换 |
|---|---|---|
| image generation | prompt、model、size/quality/background 等 | 不能转成 Chat text response；不能假设所有 Provider 返回相同 URL TTL |
| image edit/variation | 当前 edit schema 的 JSON references，以及 variation 的 multipart image；目标 Provider 可能另有 file-backed edit profile | 不能假定二者共用 body parser；不能丢掉 reference、mask、filename 或 media type |
| image input | JSON URL/data URL/file reference | 不能把用户 URL 当作网关 endpoint；不能跨 issuer 复用 file reference |

参考：[Image generation guide](https://developers.openai.com/api/docs/guides/image-generation)、[Create image endpoint](https://api.openai.com/v1/images/generations)。

## 6. 对 OpenBridge 当前架构的映射

### 6.1 当前 checkout 已证明的边界

当前源码事实如下：

| 当前事实 | 证据 |
|---|---|
| 受保护业务路由只有 `/v1/chat/completions`、`/v1/responses` 以及 Models 扩展接口 | [`src/ingress/router.rs`](../../../src/ingress/router.rs) |
| Chat/Responses handler 只接受 `application/json` | [`src/ingress/handlers.rs`](../../../src/ingress/handlers.rs) |
| `ApiProtocol` 只有 Chat Completions 和 Responses，`ApiRequest` 持有 `Bytes` body | [`src/core/request.rs`](../../../src/core/request.rs) |
| OpenAI-compatible adapter 会解析 JSON、替换受信 `model` 字段并以 JSON body 发送 | [`src/providers/openai_compatible.rs`](../../../src/providers/openai_compatible.rs) |
| shared upstream transport 的 prepared body 是 `Bytes`，响应 body 才按流传递 | [`src/transport/upstream.rs`](../../../src/transport/upstream.rs) |
| request analysis 目前只识别 image input；Chat 的 `input_audio`/`file` 和 Responses 的 `input_file` 会进入 reserved/unimplemented 拒绝路径 | [`src/pipeline/analysis.rs`](../../../src/pipeline/analysis.rs) |
| capability 类型已经预留 audio/file/custom tool 等字段，但 registry compilation 会对这些 reserved 字段 `unimplemented!` | [`src/core/capability.rs`](../../../src/core/capability.rs) |
| canonical model 已有 Audio/File/Video input modality 和 Audio/Image output modality 的枚举位置，但 `ModelMode` 当前只有 Chat | [`src/registry/definition.rs`](../../../src/registry/definition.rs) |
| registry 已按 Public Model/下游协议预编译唯一 execution interface，能力投影、preflight 与固定候选共享同一对象；当前只有 Chat/Responses 两种 interface | [`src/registry/public_model.rs`](../../../src/registry/public_model.rs) |
| 本服务 OpenAPI 只描述实际提供的 Chat/Responses/Models/health/docs endpoint | [`docs/openapi.yaml`](../../openapi.yaml) |

因此，不能仅把 `audio_input`、`file_input` 或 `InputModality::Audio` 改成 `true` 就宣称完成 embedding、File 或 voice 转发；这会绕过 body、response、resource、retry 和安全边界。

### 6.2 应分离的抽象维度

后续扩展应保持以下维度分开。具体 Rust 类型名称可以调整，但不能把它们压缩成 Chat/Responses 的几个布尔字段：

| 维度 | 至少需要表达 |
|---|---|
| Operation/endpoint | embeddings、files、transcription、translation、speech、images、Chat、Responses、Realtime |
| Request body | JSON、multipart、raw bytes、query-only、session event |
| Response body | JSON、SSE、raw audio/image/file bytes、resource lifecycle |
| Model capability | 输入/输出 modality、stream、format、dimensions、model-specific parameter support |
| Resource affinity | `file_id`、voice ID、response/conversation ID、vector store/container ID 的 issuer/Target 绑定 |
| Retry safety | 可重放的纯请求、带副作用的 create/upload、已开始输出的 stream、opaque resource mutation |
| Bridgeability | Native only、同 wire family 可转换、明确不支持；不能以“都是 OpenAI JSON”作为可转换证明 |

特别是：

- `ApiProtocol` 是 Chat/Responses 的协议分类，不应继续承担所有 OpenAI API 族的 endpoint identity。
- `GenerationCapabilities` 只适合生成请求的共同能力；embedding 的维度/编码、文件的 purpose、音频的 format/voice、二进制响应都应有专门 contract。
- `ModelConfig` 的 modality 是模型事实；endpoint capability 才是某个 Public Model 是否允许某个请求形状的公共契约。
- 现有 `ModelExecutionInterface` 是扩展的正确编译接缝：新增能力必须与其固定 candidate 列表一起编译，由 Models projection 与 preflight 共享，不能在 planning 或 forwarding 重新求值。
- File/voice/vector resource 不一定有 `model` 字段，不能强行经过“先读 model 再规划 Route”的生成请求路径。
- Dedicated endpoint 没有 Chat ↔ Responses 的通用 Bridge。没有原生上游 API 或明确的语义转换器时，应在 egress 前返回稳定 unsupported error。

### 6.3 Native、Bridge 与媒体数据

Native path 可以保留同一 endpoint family 的未知字段，但必须仍执行：

1. endpoint、model/operation 和 body media type 的受信选择；
2. request size、multipart boundary、filename、data URL 和 content part 的边界校验；
3. Provider-specific model/path/header/auth 生成；
4. response media type 与 stream lifecycle 校验；
5. resource ID、voice ID 和 state affinity 绑定。

Chat ↔ Responses bridge 只有在明确支持对应的 image/file/audio content part、顺序、编码和输出表达时才可启用。任何一项不能表达时都应拒绝，而不是把媒体转成文本、把 file ID 当 URL、或静默去掉 part。

## 7. 安全、资源和故障边界

### 7.1 输入与上游 egress

- 下游 `Content-Type`、multipart boundary、`Content-Length` 和文件名属于协议输入，但不因此获得任意 upstream URL、credential、Authorization 或 Host 的控制权。
- 远程 image/file URL 是上游可能访问的外部资源；应有独立的 URL policy、解析/重定向限制和观测脱敏规则。禁止把它们拼入 OpenBridge 的 trusted endpoint URL。
- data URL、base64 音频/图像和上传文件都需要独立的编码后/解码后大小上限；只限制 JSON 字节数不足以防止膨胀。
- 文件名、purpose、voice ID、file ID 和响应 URL 可能包含业务信息，不能进入低基数 metrics label 或普通 debug 输出。
- 不要默认把下游上传文件落盘、缓存或持久化；如果实现必须落盘，应另有临时目录、权限、清理、配额和取消契约。

### 7.2 Resource-aware fallback

| 请求 | 可接受的默认故障策略 |
|---|---|
| stateless embedding | 同一候选的受限 retry；跨 Provider fallback 需向量身份等价证明 |
| file list/retrieve/content/delete | 按 resource issuer/Target 路由；未知 issuer 直接拒绝，不跨 Target 猜测 |
| file upload、vector-store attach、voice consent | 无 idempotency/状态查询证明时，不使用通用自动 replay/fallback |
| transcription/translation/speech/image generation | 只在请求可重放、输出格式一致且尚未提交下游 body 时考虑 retry；大 body 和副作用需单独预算 |
| raw audio/image/file stream | 第一个业务字节提交后不可 fallback；正常 EOF 与上游错误必须区分 |
| Chat/Responses multimodal stream | 继续沿用现有首输出 commit、SSE terminal、取消和 Bridge failure 边界 |

## 8. 最小契约测试矩阵

新增实现前，建议把下列可观察行为分别落到 Rust contract test、canonical corpus 或经批准的外部 SDK/Provider 验收。测试通过只证明对应层，不证明真实 Provider 的全部能力。

| 族 | 必须有的确定性 case |
|---|---|
| Embeddings | 单文本、批量输入、token array、`float`/`base64`、`dimensions`、空输入/超限拒绝、model rewrite、usage/index 顺序、429/5xx retry 和跨 Provider 向量身份拒绝 |
| Files/Uploads | multipart boundary 保留、多个 part、文件大小/filename/purpose 校验、upload timeout 不盲目 replay、Upload part/complete/cancel 状态、list pagination、retrieve/delete、原始 content 下载、二进制 response header 和下游取消 |
| Transcription/translation | mp3/wav fixture、response format、multipart 字段、transcription JSON、SSE transcript fragmentation/terminal、translation 不误用 transcription stream、上游错误和取消 |
| Speech | JSON request、raw `audio/*` response、SSE audio mode、不同 output format、binary EOF、Content-Type mismatch、下游取消 |
| Chat/Responses media | text+image 顺序、URL/data URL、file reference、Chat `input_audio`、audio output、Responses `input_image`/`input_file`、unsupported part 的 egress 前拒绝、Bridge 不丢 part |
| Images | generation JSON、generation SSE、edit/variation multipart、base64/URL output、输出 URL 不被错误重写 |
| Resource affinity | file/voice/response/vector resource 绑定 issuer/Target；候选失败后不把 opaque ID 投给另一 Target；未知 resource ownership 返回稳定错误 |

Rust 运行时应负责 registry、planning、Provider contract、retry/fallback、取消和 response lifecycle；Python corpus/testkit 更适合负责 multipart/二进制/SSE framing、redacted observation 和 canonical wire comparison。OpenAI SDK、独立 Python/curl、真实 Provider、Realtime/WebSocket、负载和长时间运行属于额外验收层，不能从 deterministic fixture 推导。

## 9. 实施前复核清单

每个新 API 族进入实现前，至少重新确认：

1. 官方 endpoint 的 request content type、response media types、stream 参数和错误 schema。
2. 目标 Provider 的真实 path、model name、认证、multipart/二进制兼容、模型能力和限额。
3. Public Model 是否代表生成模型、embedding 模型或资源操作；不要用 `Chat` mode 隐藏任务类型差异。
4. `file_id`、voice ID、response ID、conversation ID 和 vector store ID 的 issuer/TTL/权限范围。
5. 请求是否可安全重放，尤其是 upload、attach、delete、voice consent 和已开始输出的媒体流。
6. Native path 是否足够；如果需要 Bridge，是否有逐字段、逐事件和逐资源的可表达性证据。
7. 新 endpoint 的 body limit、日志脱敏、header allowlist、cancel、timeout、metrics 和 OpenAPI 描述。
8. 对应的失败测试、确定性 baseline、外部 SDK/独立客户端和真实 Provider 验收分别是什么；未运行的层必须明确标记。

本专题只扩充参考事实和实施边界。任何实际 endpoint 注册、capability 开启、OpenAPI 扩展或 Provider adapter 改动，都应回到项目的 [实施计划规则](../../implementation-plans/README.md)、功能需求和当前实现说明，建立一个可观察行为后再实施。
