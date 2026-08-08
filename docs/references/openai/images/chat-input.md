# OpenAI Chat Completions 图片输入调研

## 来源、范围与快照

本文只记录 Chat Completions `messages[].content[]` 中的 `image_url` input part。Responses 图片输入和 Images API 不在本文定义。

- 官方来源：[Images and vision](https://developers.openai.com/api/docs/guides/images-vision)、[Create chat completion](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create)
- 原始资料复核日期：2026-08-04；本次结构整理未重新在线复核 format、detail 或 model capability。

## 1. Wire position

`image_url` 是 Chat user message 的 typed content part。它与 surrounding text part 的顺序、part type、URL object 和可选 `detail`
共同构成 wire 语义；不能先 OCR 或替换成文本后声称无损转发。

## 2. Source

当前资料覆盖 remote URL 与 data URL/Base64 source：

- remote URL 通常由上游获取；JSON schema 不保证 redirect、DNS/IP、最终 media type、大小或下载时限；
- data URL 扩大 request body，encoded bytes、decoded bytes、media type、图片尺寸和 decoder resource 需要独立 limit；
- format 与 `detail` domain 依 model/profile，不能从 schema existence 推断。

## 3. 与其他 operation 的边界

- Responses 使用不同的 `input_image` part；见 [Responses 图片输入](responses-input.md)；
- 图片理解不证明 Images generation/edit/variation 可用；
- Chat 图片 input 仍使用 Chat JSON request 与 Chat JSON/SSE response lifecycle。

## 4. 安全与证据边界

- URL query、图片 bytes、Base64 与完整 response 不应进入普通日志；
- 入站 URL 语法检查不能证明 Provider-side fetch 安全；
- 一个 URL sample 不证明 data URL、全部 media type、detail、size 或真实 decoder 兼容。
