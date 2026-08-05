# OpenAI 扩展协议调研索引

## 文档定位

本目录只记录 OpenAI 官方 API/SDK 资料中的 wire、transport、资源生命周期和安全边界。它不包含任何具体网关的当前实现、目标类型、TDD
计划或代码接缝。

这些文档保留 2026-07 至 2026-08 的既有资料快照；本次目录整理没有重新验证动态模型列表、限额、枚举或 beta
状态。用于兼容结论前需要重新核对官方资料并固定 SDK/文档版本。

## 协议专题

| 编号 | 专题                                                                           | 主要协议形状                                   |
|-----:|--------------------------------------------------------------------------------|------------------------------------------------|
|   01 | [Embeddings](01-embeddings.md)                                                 | 独立 JSON vector operation                     |
|   02 | [Chat/Responses 多模态 input](02-chat-responses-multimodal.md)                 | 协议内 content-part union                      |
|   03 | [Text-to-speech](03-audio-speech.md)                                           | JSON request、binary/stream audio response     |
|   04 | [Transcription/Translation](04-audio-transcription-translation.md)             | multipart upload 与多种 response format        |
|   05 | [Images](05-images.md)                                                         | generation/edit/variation 与 URL/base64 result |
|   06 | [Files](06-files.md)                                                           | multipart create 与 opaque resource lifecycle  |
|   07 | [Uploads、Vector Stores、File Search](07-uploads-vector-stores-file-search.md) | transaction、resource graph 与 hosted tool     |
|   08 | [Videos](08-videos.md)                                                         | asynchronous resource lifecycle                |
|   09 | [Realtime](09-realtime.md)                                                     | WebRTC/WebSocket 双向 session/event protocol   |

## 共同观察

- endpoint family、request encoding、response media type 和资源状态机必须分别记录。
- `file_id`、vector-store id、video id、session id 等 opaque identity 由签发服务拥有，不能假定跨 endpoint/账户可移植。
- remote URL、inline base64、multipart binary 与 hosted resource 是不同 source 形状。
- JSON/SSE、binary streaming、WebSocket/WebRTC 不能只用一个 `streaming` 布尔值概括。
- 官方 service limit、purpose、voice、format、model 和 beta 能力会变化，必须按资料日期理解。

## 官方入口

- [OpenAI API Reference](https://developers.openai.com/api/reference)
- [Embeddings](https://developers.openai.com/api/reference/resources/embeddings/methods/create)
- [Images and vision](https://developers.openai.com/api/docs/guides/images-vision)
- [File inputs](https://developers.openai.com/api/docs/guides/file-inputs)
- [Audio and speech](https://developers.openai.com/api/docs/guides/audio)
- [Image generation](https://developers.openai.com/api/docs/guides/image-generation)
- [File search](https://developers.openai.com/api/docs/guides/tools-file-search)
- [Video generation](https://developers.openai.com/api/docs/guides/video-generation)
- [Realtime](https://developers.openai.com/api/docs/guides/realtime)

