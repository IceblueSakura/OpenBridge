# OpenAI Chat/Responses JSON 多模态协议调研

## 1. 协议内 content parts

Chat Completions 与 Responses 都把多模态输入放在各自的 content-part union 中，但字段名与嵌套形状不同。

官方资料：[Chat Create](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create)、[Responses Create](https://developers.openai.com/api/reference/resources/responses/methods/create)、[Images and vision](https://developers.openai.com/api/docs/guides/images-vision)、[File inputs](https://developers.openai.com/api/docs/guides/file-inputs)、[Audio](https://developers.openai.com/api/docs/guides/audio)。

## 2. Image input

- Chat 使用 `image_url` content part，URL 对象还可带 `detail`。
- Responses 使用 `input_image`，可通过 URL、data URL 或 `file_id` 等受支持 source 引用图像。
- `detail` 的允许值和 SDK type 可能随版本演进；官方 guide 与具体 SDK release 需要一起固定。

Chat `image_url` 和 Responses `input_image` 语义相近，但不是字段级同构对象。

## 3. File input

- Chat 可使用 file content part；source 可能是 file id、inline file data 与 filename 组合。
- Responses 使用 `input_file`；source 可包括 file id、file URL 或 inline data，具体 union 以当期 schema 为准。
- File API resource 与 inline/remote content part 具有不同 ownership、TTL 和下载行为。

## 4. Audio input

Chat 可使用 `input_audio` part，通常携带 base64 audio data 与 format。它与 Audio Transcription endpoint、TTS endpoint 和 Realtime audio session 是不同协议面。

## 5. Source 与安全边界

- remote URL 可能由上游服务主动获取；其重定向、网络范围、大小与超时不由 JSON schema 本身保证；
- inline base64 同时占用 encoded body 与 decoded media budget；
- file id 是签发服务拥有的 opaque identity；
- content parts 的顺序、类型和嵌套字段是协议语义，不能先转成纯文本再声称无损；
- 多模态 input 支持不意味着 image generation、audio output 或 Realtime transport 同时可用。
