# OpenAI Embeddings 与多模态 API 关系调研

## 目的与证据边界

本文记录 Embeddings、文件、图像、音频/语音和专用媒体 endpoint
之间的协议边界。逐协议资料见[扩展协议调研索引](protocol-details/README.md)。

复核时间：2026-08-04，Asia/Shanghai。官方 schema、限额、model capability 和 beta 状态会变化；本文不是永久兼容声明。

官方入口：

- [OpenAI API Reference](https://developers.openai.com/api/reference)
- [openai/openai-openapi](https://github.com/openai/openai-openapi)
- [Images and vision](https://developers.openai.com/api/docs/guides/images-vision)
- [File inputs](https://developers.openai.com/api/docs/guides/file-inputs)
- [Audio and speech](https://developers.openai.com/api/docs/guides/audio)
- [Speech to text](https://developers.openai.com/api/docs/guides/speech-to-text)
- [Text to speech](https://developers.openai.com/api/docs/guides/text-to-speech)
- [Migrate to Responses](https://developers.openai.com/api/docs/guides/migrate-to-responses)

## 1. Endpoint family map

| API family           | 代表路径                                        | Request                           | Success response                 | Lifecycle                              |
|----------------------|-------------------------------------------------|-----------------------------------|----------------------------------|----------------------------------------|
| Embeddings           | `POST /v1/embeddings`                           | JSON                              | JSON vector list                 | 一次无会话 operation                   |
| Files                | `/v1/files`、`/{file_id}`、`/content`           | query、multipart、binary download | resource JSON 或 bytes           | opaque resource lifecycle              |
| Uploads              | `/v1/uploads`、`/parts`、`/complete`、`/cancel` | JSON + multipart parts            | upload state / final file        | 有过期与一次性 complete 的 transaction |
| Transcription        | `/v1/audio/transcriptions`                      | multipart                         | JSON/text/subtitle 或特定 stream | upload + model processing              |
| Translation          | `/v1/audio/translations`                        | multipart                         | JSON/text/subtitle               | upload + model processing              |
| Text-to-speech       | `/v1/audio/speech`                              | JSON                              | audio bytes/stream               | media response，不是 text SSE          |
| Chat multimodal      | `/v1/chat/completions`                          | JSON content parts                | Chat JSON/SSE                    | message/chunk lifecycle                |
| Responses multimodal | `/v1/responses`                                 | JSON items/content parts          | Responses JSON/typed SSE         | item/response lifecycle                |
| Images               | `/v1/images/*`                                  | JSON 或 multipart                 | URL/base64 或专用 stream         | generation/edit/variation              |
| Videos               | `/v1/videos/*`                                  | JSON 或 multipart                 | async resource / video bytes     | create/poll/download/delete            |
| Realtime             | `/v1/realtime/*`                                | WebRTC/WebSocket/session HTTP     | 双向 media/events                | long-lived session                     |

相同的 `model` 字段或 `stream` 概念不能消除这些 request encoding、response media type 和状态机差异。

## 2. Embeddings 的独立语义

Embeddings response 是按 `index` 对齐 input 的向量列表，并带 model 与 input/total usage。它没有 assistant role、tool
item、completion token 或 Responses terminal event。

需要分别记录：

- input form：单文本、文本批次、token input；
- output encoding：float/base64；
- dimension domain；
- batch/input token/response size limit；
- vector identity 与模型版本。

目录把某模型标为 embedding mode，不单独证明每种 input、encoding 或 dimensions 都可用。

## 3. Chat 与 Responses 多模态 input

两种生成 API 都支持 content-part union，但字段不完全同构：

| Source            | Chat                                 | Responses                                   |
|-------------------|--------------------------------------|---------------------------------------------|
| Image URL/data    | `image_url` part                     | `input_image` part                          |
| Hosted image/file | Chat file/image union 允许的 id 形状 | `input_image` / `input_file` 的 `file_id`   |
| Remote file URL   | 取决于 Chat file schema/profile      | `input_file.file_url` 等 union              |
| Inline file       | file data + filename                 | file data + filename                        |
| Audio input       | `input_audio`                        | 由 Responses 当前 schema/model profile 决定 |

part type、source、detail、format、filename、顺序和嵌套字段都是 wire 语义。将所有内容先转成文本会丢失来源与媒体信息。

## 4. Remote、inline 与 hosted resource

- **Remote URL**：上游服务可能主动获取资源；redirect、DNS/IP 范围、大小、media type 和下载时限不由 JSON schema 自动保证。
- **Inline data**：base64 增加传输体积，decode 后还需要独立 bytes/media limit。
- **Hosted resource**：file/vector-store/video id 受签发项目、账户、Provider 与 TTL 约束。

三种 source 不能只压缩为一个 `supports_image` 或 `supports_file` 布尔值。

## 5. Audio 与 voice

Chat audio input、Transcription、Translation、TTS 和 Realtime 分别具有不同 transport：

- Chat audio 是 JSON content part；
- transcription/translation 是 multipart upload；
- TTS 返回 binary audio 或专用 stream；
- Realtime 是双向 session/event protocol。

voice、audio format、sample encoding、VAD、transcript 和 media streaming 都需要按 endpoint/model 分别理解。

## 6. Images 与 Videos

Images generation/edit/variation 是独立 endpoint family。结果可以是短期 URL、base64 或特定 streaming event。Videos 是异步
resource workflow，需要 poll 状态并单独下载 content。

这两类 operation 可能已经被服务接受并产生费用；网络结果不确定时不能只根据客户端未收到响应就假定安全重放。

## 7. Resource identity 与 state

File、Upload、Vector Store、Video、Realtime session 与 Responses continuation 分别签发不同 opaque identity。跨 endpoint、账户或
Provider 的可移植性必须由正式 contract 明确，不能由字符串形状推断。

## 8. 共同安全与证据边界

- binary、base64、remote URL 和 signed download URL 不应进入普通日志或 metric label；
- service limit、purpose、format、voice、model 和 enum 会变化；
- SDK helper type 不能替代 wire schema；SDK 与 API reference 不一致时需要固定具体版本；
- 单个成功 sample 不证明全部 model、source、format、stream、error 或 cancel 行为；
- 专用 media/resource API 的存在不意味着 Chat/Responses 同时支持等价能力。
