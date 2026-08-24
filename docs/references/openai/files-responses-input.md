# OpenAI Responses 文件输入调研

## 来源、范围与快照

本文只记录 Responses message content 中的 `input_file` part。Chat file、Files/Uploads resource 和 File Search tool 分别由其他文档
维护。

- 官方来源：[File inputs](https://developers.openai.com/api/docs/guides/file-inputs)、[Create a response](https://developers.openai.com/api/reference/resources/responses/methods/create)
- 原始资料复核日期：2026-08-21；本次重新核对当前官方 API reference 与 File inputs guide。

## 1. Wire position 与 source one-of

`input_file` 位于 Responses ordered item/content 结构。当前 `input_file` source union 包含 inline `file_data`、external `file_url` 与 hosted `file_id`。
PDF part 可选 `detail: auto|low|high`，省略时默认 `auto`；该字段只影响 PDF page image 处理，
Chat file part 不支持它。

同时携带多个互斥 source、丢失 filename/encoding 或把 part 转为 `input_text` 都会改变 wire 语义。

## 2. Source 边界

- inline data 需要 encoded/decoded bytes、filename、format/media type 与 parser resource limit；
- remote URL 由上游 fetch 时，redirect、DNS/IP、最终 MIME、大小与时限不由 JSON schema 保证；
- `file_id` 绑定 issuer、账户/项目、purpose、权限与 retention。

## 3. 与其他 operation 的边界

- Chat file 见 [Chat 文件输入](files-chat-input.md)，两者不是字段级同构；
- Files create/download/delete、Uploads 和 Vector Stores 不由 `input_file` capability 自动提供；
- File Search 是 Responses hosted tool，见 [Responses File Search](files-responses-file-search.md)。

## 4. 数据与证据边界

- URL query、filename、file id、bytes 与 Base64 可能敏感；
- 一个 source success 不证明其他 source、state affinity、格式、大小或 Provider fetch；
- fixture 只证明被覆盖的 JSON shape。
