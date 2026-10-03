# OpenAI 协议来源索引

## 文档定位

本目录定位 OpenAI 官方 API/SDK 的标准资料与既有固定基线。这里不记录 OpenBridge 当前实现、目标类型、
实施计划或已运行验证。

正文按“具体 API operation + request encoding + response transport”划分：

- 一个协议事实只有一个 owner 文档；
- JSON、multipart、binary、SSE 与双向 session 不混写为一种通用 request/streaming 能力；
- Chat、Responses、专用 media endpoint 与 hosted tool 即使处理同一模态，也分别记录；
- 只有共享同一 opaque identity/state machine 的 lifecycle operation 才放在同一文档；
- 本索引和 [API 规范导航](api-specification-catalog.md)只定位来源，不复制字段级协议说明或动态 endpoint 库存。

动态 model、limit、enum、voice、format、tool 与 beta 状态必须按各文档快照日期理解，形成兼容结论前重新复核官方资料并固定目标
SDK/文档版本。

## 1. Chat Completions

| 主题              | Request/response 形式               | Owner 文档                                                        |
|-------------------|-------------------------------------|-------------------------------------------------------------------|
| Create request    | `POST /v1/chat/completions` JSON     | [JSON request](chat-completions-request.md)                        |
| Non-stream result | JSON `chat.completion`               | [非流式响应](chat-completions-non-streaming-response.md)           |
| Streaming result  | data-only SSE chunks                | [Chat SSE](chat-completions-streaming.md)                          |
| Client tools      | JSON call/result round trip         | [Function tools](chat-completions-function-tools.md)               |
| Structured output | `response_format` JSON              | [Structured output](chat-completions-structured-output.md)         |
| Stored resources  | list/retrieve/update/delete/messages | [Stored Chat resources](chat-completions-stored-resources.md)      |

## 2. Responses

| 主题                | Request/response 形式                         | Owner 文档                                                   |
|---------------------|-----------------------------------------------|--------------------------------------------------------------|
| Create request      | `POST /v1/responses` JSON                     | [Create JSON request](responses-request.md)                   |
| Non-stream result   | JSON `response` + ordered `output[]`          | [非流式响应](responses-non-streaming-response.md)             |
| Streaming result    | typed semantic SSE                           | [Responses SSE](responses-streaming.md)                       |
| WebSocket mode      | persistent typed JSON event connection       | [Responses WebSocket](responses-websocket.md)                 |
| Client tools        | item/call/result round trip                  | [Function tools](responses-function-tools.md)                 |
| Continuation        | response chain、conversation、manual replay  | [State ownership](responses-state.md)                         |
| Resource operations | background/retrieve/delete/cancel/compact/token ops | [Resource lifecycle](responses-resource-lifecycle.md)  |
| Structured output   | `text.format` JSON                           | [Structured output](responses-structured-output.md)           |

## 3. Embeddings

| Operation          | Request/response 形式 | Owner 文档                                  |
|--------------------|-----------------------|---------------------------------------------|
| Embeddings create  | JSON → JSON vector list | [Embeddings Create](embeddings-create.md)   |

## 4. 图片

| Operation                   | Request/response 形式                 | Owner 文档                                                    |
|-----------------------------|---------------------------------------|---------------------------------------------------------------|
| Chat image input            | Chat JSON `image_url` part            | [Chat 图片输入](images-chat-input.md)                         |
| Responses image input       | Responses JSON `input_image` part     | [Responses 图片输入](images-responses-input.md)               |
| Images generations          | JSON → URL/Base64 或专用 stream       | [Images Generations](images-generations.md)                   |
| Images edits/variations     | multipart → URL/Base64 result         | [Images Edits/Variations](images-edits-and-variations.md)     |
| Responses hosted generation | hosted-tool item/event lifecycle     | [Hosted image generation](images-responses-hosted-generation.md) |

## 5. 文件与检索资源

| Operation                 | Request/response 形式                       | Owner 文档                                              |
|---------------------------|---------------------------------------------|---------------------------------------------------------|
| Chat file input           | Chat JSON file part                         | [Chat 文件输入](files-chat-input.md)                    |
| Responses file input      | Responses JSON `input_file` part            | [Responses 文件输入](files-responses-input.md)          |
| Files create              | multipart → File metadata                   | [Files Create](files-create.md)                         |
| Files list/retrieve/delete | JSON/resource operation                    | [Metadata 与 delete](files-metadata-and-delete.md)      |
| Files content             | binary download                             | [Content download](files-content-download.md)           |
| Uploads                   | JSON + multipart transaction                | [Uploads transaction](files-uploads-transaction.md)     |
| Vector Stores             | resource + processing lifecycle             | [Vector Stores](files-vector-stores.md)                 |
| Responses File Search     | hosted-tool item/result                     | [File Search](files-responses-file-search.md)           |

## 6. 音频与语音

| Operation            | Request/response 形式                         | Owner 文档                                             |
|----------------------|-----------------------------------------------|--------------------------------------------------------|
| Speech               | JSON → binary/stream audio                    | [Audio Speech](audio-speech.md)                        |
| Transcriptions       | multipart audio → JSON/text/subtitle/stream   | [Audio Transcriptions](audio-transcriptions.md)        |
| Translations         | multipart audio → translation result          | [Audio Translations](audio-translations.md)            |
| Custom voices        | multipart consent/sample → voice resources    | [自定义声音与 consent](audio-custom-voices.md)         |
| Chat audio in/out    | Chat JSON / data-only SSE                     | [Chat 音频输入/输出](audio-chat-input-output.md)       |

## 7. Realtime

| Operation          | Request/response 形式                         | Owner 文档                                      |
|--------------------|-----------------------------------------------|-------------------------------------------------|
| HTTP control plane | client-secret/call/signaling resources        | [Realtime control plane](realtime-control-plane.md) |
| Session/data plane | 对话、翻译、转写的 WebRTC/WebSocket/SIP events | [Realtime transport](realtime-transport.md)         |

## 8. 视频

| Operation                  | Request/response 形式                          | Owner 文档                                         |
|----------------------------|------------------------------------------------|----------------------------------------------------|
| Videos create              | JSON/multipart → async resource                | [Videos Create](videos-create.md)                  |
| Video lifecycle            | list/retrieve/status/download/delete           | [Resource lifecycle](videos-resource-lifecycle.md) |
| Edit/extension/remix       | source media/resource → new async resource     | [派生 Video jobs](videos-derived-jobs.md)          |
| Characters                 | multipart sample → character resource          | [Video characters](videos-characters.md)           |

> **时效边界：**截至 2026-08-10，官方指南已将 Sora 2 Videos API 及其 models 标为 deprecated，并计划于
> **2026-09-24** 关闭。本组 Videos 文档用于保存关闭前的协议快照，不构成长期兼容承诺；任何实现工作开始前必须重新确认替代 API。

## 9. 端点采用、Moderations 与 fake 边界

| 主题 | Request/response 形式 | Owner 文档 |
|------|-----------------------|------------|
| Operation 与验证边界 | 区分 task、encoding、transport、lifecycle 与证据 | [采用与验证边界](endpoint-adoption-and-fake-testing.md) |
| Moderations create | `POST /v1/moderations` JSON → JSON classifications | [Moderations Create](moderations-create.md) |

跨 operation 文档只保留采用边界；字段级事实仍由各 operation owner 维护，验证方法归[验收基线](../conformance-baseline.md)。这些来源不构成产品范围、当前实现或获准计划。

测试方法见[验收基线](../conformance-baseline.md)，本地执行入口见[开发指南](../../development.md)。不保存外部测试资产调查或执行结果；引入资产前另行核对版本、许可和独立预期。
