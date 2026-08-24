# OpenAI Chat Completions 文件输入调研

## 来源、范围与快照

本文只记录 Chat Completions message content 中的 file part。Responses `input_file`、Files API、Uploads 和 File Search 不在本文定义。

- 官方来源：[File inputs](https://developers.openai.com/api/docs/guides/file-inputs)、[Create chat completion](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create)
- 原始资料复核日期：2026-08-21；本次重新核对当前官方 API reference 与 File inputs guide。

## 1. Wire position 与 source

Chat file 是 user message 的 typed content part。当前 API reference 将 Chat `file` 表示为嵌套 `file` object，source 是 `file_data` 或 hosted
`file_id`；inline `file_data` 需要 `filename`。Chat file part 不接受 `file_url` 或 `detail`。

part type、filename、data/id 与 surrounding content 顺序属于 wire 语义。把文件提取成 text 会丢失 source、格式、结构和 identity。

## 2. Source 边界

- inline Base64 同时占用 encoded body 与 decoded file budget；
- filename、format/media type 和 parser resource 需要独立限制；
- hosted file id 绑定签发服务、账户/项目、purpose、权限与 retention，不能跨 Provider 猜测迁移。

## 3. 与其他 operation 的边界

- Responses 使用不同的 `input_file`；见 [Responses 文件输入](files-responses-input.md)；
- Files create/resource、Uploads 与 Vector Store membership 是独立 lifecycle；
- file input success 不证明 File Search hosted tool 可用。

## 4. 数据与证据边界

- filename、file id、原始 bytes 与 Base64 可能敏感；
- 一个 inline sample 不证明 hosted id、全部 format、size 或 model support；
- SDK helper 不能替代 Chat JSON content-part wire。
