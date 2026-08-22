# OpenAI Images Generations 调研

## 来源、范围与快照

本文只记录 Images API generation operation 的 JSON request、URL/Base64 result 与 generation-specific stream。Edit/variation、
Chat/Responses 图片输入和 Responses hosted tool 不在本文定义。

- 官方来源：[Image generation](https://developers.openai.com/api/docs/guides/image-generation)、[Images API](https://developers.openai.com/api/reference/resources/images)
- 原始资料复核日期：2026-08-22；标准 Create 字段、response metadata 与 generation stream 在本焦点重新在线复核。

## 1. Request

generation 为 JSON request。当前标准字段包括 `model`、`prompt`、`n`、`size`、`quality`、`style`、
`response_format`、`output_format`、`output_compression`、`background`、`moderation`、`stream`、`partial_images` 与 `user`。
字段是否可用取决于 endpoint 与 model/profile，不能从一个 model 推断到全部 Images model；optional `null` 代表省略。
Provider 私有扩展不是标准字段，gateway 必须显式建模，不能任意 passthrough。

## 2. Result

success 可返回短期 URL 或 Base64 image data：

- URL 可能带 signed query 与短 TTL，不是永久 resource identity；
- Base64 扩大 JSON body，并需要独立 decode/media budget；
- response format 与 media container 必须按目标 profile 记录。

## 3. Generation stream

特定 profile 可提供 progress 或 partial-image event。该 stream 不是 Chat data-only chunks，也不是 Responses text SSE。terminal、EOF、
取消和累计媒体 bytes 必须按 Images generation event grammar 单独验证。

## 4. Retry、安全与证据边界

- request 可能已经被接受、计费或产生结果，网络结果不确定时不能盲目重放；
- prompt、Base64、signed URL 和 partial image 不应进入普通日志；
- 一个 generation success 不证明 edit、variation、全部 format、stream 或 error path。
