# OpenAI Chat Completions SSE 调研

## 来源、范围与快照

本文只记录 `POST /v1/chat/completions` 使用 `stream: true` 时的 data-only SSE response 与 chunk 累积规则。

- 官方来源：[Chat API reference](https://developers.openai.com/api/reference/resources/chat)、[Streaming responses](https://developers.openai.com/api/docs/guides/streaming-responses)
- 协议复核日期：2026-08-11；本次只复核 `stream_options.include_usage` 与 usage 尾块，不穷举其他 event 扩展。

## 1. Chunk shape

每个 data payload 的核心对象是 `chat.completion.chunk`：

```json
{
  "object": "chat.completion.chunk",
  "choices": [{
    "index": 0,
    "delta": {"content": "partial"},
    "finish_reason": null
  }]
}
```

chunk 使用 `choices[].delta`，不是非流式的 `choices[].message`。delta 可包含 role、文本片段、tool-call name/arguments 片段，
也可能为空。

## 2. Accumulator

1. 为每个 `choice.index` 建立独立 accumulator；
2. 将 `delta.content` 追加到对应 choice；
3. tool call 还需按其 index/id 分别累积 name 与 arguments；
4. arguments delta 是字符串片段，完成前不能逐片当成完整 JSON parse；
5. 非空 `finish_reason` 只结束对应 choice；
6. 只有所有需要的 choice 完成且 stream 正常终止后，才能构造完整 completion view。

## 3. Usage、terminal 与失败

`stream_options.include_usage:true` 只与 `stream:true` 组合。启用后，普通 chunks 的 `usage` 为 `null`；在 `[DONE]` 前额外出现一个
`choices:[]` 且携带整次请求 usage 的 chunk。流在到达 terminal 前中断时可能收不到该尾块，因此消费者不能把缺失 usage 自动补零，
也不能把 finish chunk 当成 usage chunk。

Chat stream 是 data-only SSE，不具有 Responses 的 typed response/item/content event grammar。transport EOF、SSE parse error、非 2xx
upstream error与正常 terminal 必须分开；partial text 不能在异常 EOF 后伪装成完整成功。

## 4. 证据边界

- 工具 arguments 的调用闭环见 [Function tools](function-tools.md)；本文只拥有分片累积规则；
- Chat chunk 不能只改 event 名称就成为 Responses typed event；
- SDK helper 可验证特定版本的 consumer 行为，但不能证明 gateway、Provider 或跨协议转换完整兼容。
