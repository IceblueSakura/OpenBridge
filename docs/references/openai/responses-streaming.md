# OpenAI Responses typed SSE 调研

## 来源、范围与快照

本文只记录 `POST /v1/responses` 使用 `stream: true` 时的 response/item/content/text 核心 typed SSE grammar。工具参数、图片生成
等专用 delta 由对应 operation 文档维护。

- 官方来源：[Streaming responses](https://developers.openai.com/api/docs/guides/streaming-responses)、[Streaming events reference](https://developers.openai.com/api/reference/resources/responses/streaming-events)
- 协议复核日期：2026-08-11；本次只复核成功 terminal usage，不穷举完整 event catalog。

## 1. Core lifecycle

典型 text lifecycle：

```text
response.created
response.in_progress
response.output_item.added
response.content_part.added
response.output_text.delta*
response.output_text.done
response.content_part.done
response.output_item.done
response.completed
```

Responses event 带 `type` 与语义字段，不是 Chat data-only chunk。item done、content part done 与整个 response terminal 是不同层次。

## 2. Assembler

1. 用 `response.id` 建立 response state；
2. 用 `output_index` 与 item id 建立独立 output item state；
3. `output_item.added` 创建 item，content/text delta 只更新对应 buffer；
4. `output_item.done` 只结束一个 item；
5. `response.completed`、`response.incomplete`、`response.failed` 或 `response.cancelled` 结束 response lifecycle；
6. terminal event 中的 response status、usage、error 仍需完整读取；成功 `response.completed.response.usage` 使用
   `input_tokens`、`output_tokens`、`total_tokens` 及可选 details，不能从中间 delta 自行估算。

## 3. Error 与 EOF

顶层 `error` event、`response.failed`、非 2xx HTTP、SSE parse error 和 transport EOF 是不同失败形状。terminal 前 EOF 不能被补造成
`response.completed`，已收到的 partial text 也不能冒充完整成功。

## 4. 专用 event 归属

- Function/custom tool arguments delta：见 [Responses Function tools](responses-function-tools.md)；
- image generation progress/partial output：见 [Responses hosted image generation](images-responses-hosted-generation.md)；
- file search call/result：见 [Responses File Search](files-responses-file-search.md)；
- Realtime 双向 events：见 [Realtime transport](realtime-transport.md)，不属于 Responses SSE。

## 5. 证据边界

- 官方 event catalog 会随 tool 与 model capability 演进，兼容结论必须固定快照；
- Chat chunk handler 不能通过重命名直接消费 Responses events；
- 本文不把任何客户端或 Provider 私有 event 写成公开 Responses 契约。
