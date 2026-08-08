# OpenAI Chat Completions Function tools 调研

## 来源、范围与快照

本文只记录 Chat Completions 的 client-executed function tool 定义、assistant tool call 与 `role: "tool"` 结果闭环。内建工具和
Provider 私有工具不是本文的等价扩展。

- 官方来源：[Function calling](https://platform.openai.com/docs/guides/function-calling)、[Create chat completion](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create)
- 协议复核日期：2026-08-03；本次结构整理未重新在线复核 tool kind 或 model capability。

## 1. Request 控制

`tools[]` 定义 function/custom tool，`tool_choice` 控制禁止、自动、要求或指定工具，`parallel_tool_calls` 控制并行调用能力。
tool type、name、description、parameters/schema、strict 与 choice 共同决定行为，不能把它们当作展示 metadata。

## 2. Assistant call

典型非流式 assistant message：

```json
{
  "role": "assistant",
  "content": null,
  "tool_calls": [{
    "id": "call_abc",
    "type": "function",
    "function": {
      "name": "get_weather",
      "arguments": "{\"city\":\"Paris\"}"
    }
  }]
}
```

`arguments` 是不可信的模型输出字符串。应用必须完成 JSON parse、schema validation、授权与错误处理后，才可执行本地工具。

## 3. Tool result 与下一轮

```json
{
  "role": "tool",
  "tool_call_id": "call_abc",
  "content": "{\"temperature_c\":25}"
}
```

`tool_call.id`/`tool_call_id` 是关联键，不是 message id、choice index 或 stream chunk index。多个调用可并行出现，客户端必须保留完整
assistant tool-call message，再按对应 id 追加每个结果。

## 4. Streaming 边界

SSE 中 tool name 与 arguments 可分布在多个 delta。consumer 应按 choice 和 tool-call index/id 累积，直到对应调用完成后再 parse。
具体 chunk grammar 见 [Chat SSE](streaming.md)。

## 5. 证据边界

- `functions`/`function_call` 是历史兼容面，不应成为新 canonical representation 的主形状；
- Chat function tool 与 Responses hosted tool/client tool 不是自动等价；
- 单个 function call 成功不证明 parallel calls、任意 arguments 分片、错误返回或多轮闭环。
