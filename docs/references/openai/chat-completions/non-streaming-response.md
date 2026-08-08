# OpenAI Chat Completions 非流式响应调研

## 来源、范围与快照

本文只记录 `POST /v1/chat/completions` 在 `stream` 省略或为 `false` 时的 JSON success response。SSE response、请求字段与
工具往返分别由其他文档维护。

- 官方来源：[Create chat completion](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create)
- 协议复核日期：2026-08-03；本次结构整理未重新在线复核动态枚举。

## 1. Response envelope

```json
{
  "id": "chatcmpl_...",
  "object": "chat.completion",
  "created": 0,
  "model": "gpt-5.6",
  "choices": [
    {
      "index": 0,
      "message": {"role": "assistant", "content": "..."},
      "finish_reason": "stop",
      "logprobs": null
    }
  ],
  "usage": {
    "prompt_tokens": 0,
    "completion_tokens": 0,
    "total_tokens": 0
  }
}
```

`choices[]` 是有序候选集合，数量可受 `n` 等请求语义影响。协议消费者不能天然假设只有 choice 0，也不能丢弃 `index`。

## 2. Message 与 terminal 语义

- `message.content` 不保证是非空文本；response 也可能表达 refusal、tool calls 或 profile-specific output；
- 常见 `finish_reason` 包括 `stop`、`length`、`tool_calls`、`content_filter`，旧 `function_call` 属于兼容值；
- 未知未来 `finish_reason` 应作为未知枚举保留，不应使 parser 崩溃；
- `finish_reason: "tool_calls"` 表示客户端还需执行工具并提交下一轮，不是最终文本答案。

工具 response 的完整关联规则见 [Function tools](function-tools.md)，音频 response 形状见
[Chat 音频输入/输出](../audio/chat-input-output.md)。

## 3. Usage 与投影边界

Chat usage 使用 prompt/completion/total token 命名，并可能带详情字段。它不能机械改名为 Responses input/output usage，也不能从缺失字段
虚构 token 数据。

兼容层可以提供方便读取的 choice 0 文本 view，但必须把它与完整 `choices[]`、message、finish reason、usage 分开保存。

## 4. 证据边界

- 本文不定义 data-only SSE chunk；见 [Chat SSE](streaming.md)；
- 单个 JSON success 不证明全部 choice、refusal、tool、audio、logprob 或错误形状；
- mock response 只能证明被断言的 JSON shape，不证明真实 model 或 SDK 兼容。
