# OpenAI API 规范目录

## 来源、范围与快照

本文只保存 API family/path 的发现地图，帮助定位应阅读的细粒度协议文档；它不重复 request field、response schema、stream grammar 或
resource state machine。

- 初次采集日期：2026-07-18；扩展 endpoint 复核日期：2026-08-04；
- 机器可读规范：官方 endpoint catalog，OpenAPI `3.1.0`，当时 `info.version=2.3.0`；
- 2026-08-04 快照读取到 182 个 endpoint path；数字只标识当次动态在线目录；
- 官方来源：[openai/openai-openapi](https://github.com/openai/openai-openapi)、[OpenAI API Reference](https://developers.openai.com/api/reference)。

全部 owner 文档见 [OpenAI 协议调研索引](README.md)。

## 1. 阅读证据层次

| 层                     | 适合回答的问题                                                 |
|------------------------|----------------------------------------------------------------|
| OpenAPI 3.1            | path、method、基础参数/response schema、生成客户端             |
| API Reference          | 字段语义、枚举、resource method 与 error                       |
| Guides                 | streaming、tool loop、state、background、media 使用方式       |
| Model docs/changelog   | 某 model 当前支持的参数、模态、format、voice 或 beta feature  |

OpenAPI shape 不能替代 model capability、tool lifecycle、resource retention 或 transport semantics。

## 2. Endpoint family map

| Family                    | 代表 path                                                                    | 细粒度入口                                    |
|---------------------------|-------------------------------------------------------------------------------|-----------------------------------------------|
| Chat Completions          | `/chat/completions`                                                           | [Chat 文档组](README.md#1-chat-completions)   |
| Responses                 | `/responses`、`/responses/{id}/*`                                             | [Responses 文档组](README.md#2-responses)     |
| Embeddings                | `/embeddings`                                                                 | [Embeddings](embeddings/create.md)            |
| Images                    | `/images/generations`、`/images/edits`、`/images/variations`                  | [图片文档组](README.md#4-图片)                |
| Files/Uploads             | `/files/*`、`/uploads/*`                                                       | [文件文档组](README.md#5-文件与检索资源)      |
| Vector Stores             | `/vector_stores/*`                                                            | [Vector Stores](files/vector-stores.md)       |
| Audio                     | `/audio/speech`、`/audio/transcriptions`、`/audio/translations`               | [音频文档组](README.md#6-音频与语音)          |
| Realtime                  | `/realtime/*` 与 WebRTC/WebSocket/SIP                                         | [Realtime 文档组](README.md#7-realtime)       |
| Videos                    | `/videos/*`                                                                   | [视频文档组](README.md#8-视频)                |
| Conversations             | `/conversations/*`                                                            | [Responses state](responses/state.md)         |
| Moderations               | `/moderations`                                                                | 尚无本目录细节页，使用前需单独调研            |
| Models                    | `/models`、`/models/{model}`                                                   | 尚无本目录细节页，不能当 capability/price 源 |
| Batch/Fine-tuning/Evals   | `/batches/*`、`/fine_tuning/*`、`/evals/*`                                    | 尚无本目录细节页                              |
| Containers/Skills/ChatKit | `/containers/*`、`/skills/*`、`/chatkit/*`                                    | 尚无本目录细节页                              |
| Organization/Projects     | `/organization/*`、`/projects/*`                                              | 管理面，尚无本目录细节页                      |
| Assistants/Threads        | `/assistants/*`、`/threads/*`                                                  | deprecated migration surface，需单独复核      |
| Legacy Completions        | `/completions`                                                                | legacy surface，尚无本目录细节页              |

## 3. 使用边界

- catalog 中存在 path 不代表本项目、目标 Provider、账户或 model 已支持；
- beta path、dynamic enum 与 SDK surface 使用前需要固定日期/版本；
- SDK method 是 HTTP API 的语言封装，不是独立 wire authority；
- endpoint family 的成功 sample 不能外推到同模态的其他 operation；
- 本目录没有细节页的 family 不应从这张地图推导实现承诺。
