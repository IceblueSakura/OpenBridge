# OpenAI 扩展协议实现细节索引

## 文档定位

本目录把 OpenAI 扩展 API 按独立 wire contract 和状态模型拆开，供 OpenBridge 后续实现时逐项建立需求、失败测试与 Provider 证据。它们是协议与实现接缝参考，不代表当前 checkout 已经提供对应 endpoint。

**复核时间：** 2026-08-04（Asia/Shanghai）。协议事实以当日 OpenAI API Reference、官方 guides 和 endpoint OpenAPI schema 为准；模型支持、限制和 beta 状态在真正实施前必须再次复核。

## 协议拆分与目标状态

| 编号 | 协议族 | 文档 | 当前决策 |
|---|---|---|---|
| 1 | Embeddings | [Embeddings](01-embeddings.md) | 已批准扩展目标；唯一当前开发焦点 |
| 2 | Chat/Responses JSON 多模态 | [Chat/Responses 多模态](02-chat-responses-multimodal.md) | 已批准扩展目标；等待 1 收口后进入单独焦点 |
| 3 | Text-to-speech | [Speech](03-audio-speech.md) | 参考储备，不在本轮目标范围 |
| 4 | Transcription/Translation | [音频转写与翻译](04-audio-transcription-translation.md) | 参考储备，不在本轮目标范围 |
| 5 | Images | [Images](05-images.md) | 参考储备；已从现阶段实施目标移除 |
| 6 | Files | [Files](06-files.md) | 参考储备；已从现阶段实施目标移除 |
| 7 | Uploads/Vector Stores/File Search | [托管文件检索资源](07-uploads-vector-stores-file-search.md) | 参考储备，不在现阶段目标范围 |
| 8 | Videos | [Videos](08-videos.md) | 参考储备，不在本轮目标范围 |
| 9 | Realtime | [Realtime](09-realtime.md) | 参考储备，不在本轮目标范围 |

编号保留协议评估时的稳定引用，不授权并行实现。现阶段只批准 1/2；仓库只允许 [`current-focus.md`](../../../implementation-plans/current-focus.md) 保存一个正在实施的可观察行为。完成 1 并把证据写入 implementation status 后，才可把 2 转成新的当前焦点。

## 跨协议共同约束

每个实现都必须显式回答以下问题，不能因为 endpoint 出现在官方 OpenAPI 中就继承 Chat/Responses 的现有行为：

1. operation identity、HTTP method、请求 media type、响应 media type 和 stream terminal 分别是什么；
2. Public Model、资源 endpoint 和无 `model` 操作如何进入 registry，且不暴露 Provider/Target/credential；
3. Native 转发需要改写哪些受信字段，哪些未知字段可保留，哪些字段必须在 egress 前拒绝；
4. 请求是否可安全重放，首个业务字节或资源创建后是否仍允许 retry/fallback；
5. `file_id`、`upload_id`、`vector_store_id`、`video_id`、voice 或 session 由谁签发，如何保持 issuer/Target affinity；
6. body、解码后媒体、multipart part、事件和下载分别采用什么有界限制；
7. Rust contract、canonical corpus、OpenAI SDK、独立客户端和真实 Provider 各证明哪一层。

Native 只表示下游与上游属于同一协议族，不表示可以跳过能力、媒体、安全或资源归属校验。Bridge 只有在每个字段、事件与资源语义都可表达时才成立；本目录没有为专用媒体或资源 API 假定通用 Bridge。

## 共同的当前代码接缝

当前实现只注册 Chat Completions、Responses 和 Models：

- [`ApiProtocol`](../../../../src/core/request.rs) 只有 Chat Completions 与 Responses；
- [`build_router`](../../../../src/ingress/router.rs) 没有本目录中的专用 endpoint；
- [`handlers.rs`](../../../../src/ingress/handlers.rs) 的业务 handler 只接受 JSON；
- [`openai_compatible.rs`](../../../../src/providers/openai_compatible.rs) 只选择 Chat/Responses path，并固定构造 JSON 请求；
- [`definition.rs`](../../../../src/registry/definition.rs) 的 Upstream API capability 和 transport 仍只覆盖两种生成协议；
- [`analysis.rs`](../../../../src/pipeline/analysis.rs) 会把尚未实现的 audio/file 请求形状在 egress 前拒绝。

因此，新增 endpoint 不能只靠打开一个已有 capability 布尔值完成。实现时至少要同步 operation/registry、ingress body、Provider path/auth、transport response、planning/retry、OpenAPI 与协议级测试。

## 官方入口

- [OpenAI API Reference](https://developers.openai.com/api/reference)
- [Embeddings API](https://developers.openai.com/api/reference/resources/embeddings/methods/create)
- [Images and vision](https://developers.openai.com/api/docs/guides/images-vision)
- [File inputs](https://developers.openai.com/api/docs/guides/file-inputs)
- [Audio and speech](https://developers.openai.com/api/docs/guides/audio)
- [Image generation](https://developers.openai.com/api/docs/guides/image-generation)
- [File search](https://developers.openai.com/api/docs/guides/tools-file-search)
- [Video generation](https://developers.openai.com/api/docs/guides/video-generation)
- [Realtime and audio](https://developers.openai.com/api/docs/guides/realtime)
