# OpenAI Responses Function tools 调研

## 来源、范围与快照

本文只记录 Responses 的 client-executed function/custom tool 定义、output call item、下一次 request result item 及其 streaming delta。
hosted tools 分别由各自 operation 文档维护。

- 官方来源：[Function calling](https://platform.openai.com/docs/guides/function-calling)、[Create a response](https://developers.openai.com/api/reference/resources/responses/methods/create)
- 协议复核日期：2026-08-03；本次结构整理未重新在线复核 tool kind 或 model capability。

## 1. Request 控制

`tools`、`tool_choice`、`parallel_tool_calls` 与 `max_tool_calls` 控制可用工具、选择策略和调用数量。Responses 的 tool union 比 Chat
function tool 更广；本文只拥有由客户端执行的 function/custom tool 语义。

## 2. Output call item

model 可在 `output[]` 中返回 `function_call` 或 `custom_tool_call`。`call_id` 是后续结果的关联键；item id、array index 与
`call_id` 不能互换。

arguments/input 是不可信模型输出。客户端必须完成 parse、schema validation、授权与错误处理后才可执行工具。

## 3. 下一次 request result item

典型 function result：

```json
{
  "type": "function_call_output",
  "call_id": "call_abc",
  "output": "{\"temperature_c\":25}"
}
```

继续工具循环时，不能丢失原 prompt、工具定义、model 生成的 call item 或对应 result item。并行调用按 `call_id` 关联，不能依赖
`output[]` 位置。

## 4. Streaming delta

`response.function_call_arguments.delta/done` 和 custom tool input delta/done 需要按 item/call state 累积。arguments 完成前不能逐片
parse JSON；item done 也不等于整个 response terminal。核心 response lifecycle 见 [Responses SSE](streaming.md)。

## 5. Hosted tool 边界

web/file search、code interpreter、computer use、image generation、remote MCP 等 hosted tools 由服务执行，拥有各自 request、result、
cost 与 lifecycle。它们不能降级为 client function tool，也不能由 function schema 的存在推断可用。

## 6. 证据边界

- Chat `tool_call_id` 与 Responses `call_id` 不是同一 wire field；
- 一个 function call 不证明 parallel call、custom tool、任意 SSE fragmentation 或 hosted tool；
- 工具执行成功不等于模型最终 response 已 completed。
