# OpenAI Responses 非流式响应调研

## 来源、范围与快照

本文只记录 `POST /v1/responses` 非流式 JSON success object、`output[]` 与 response status。SSE event、tool result request 和
resource retrieve/cancel 分别由其他文档维护。

- 官方来源：[Create a response](https://developers.openai.com/api/reference/resources/responses/methods/create)
- 协议复核日期：2026-08-03；本次结构整理未重新在线复核动态 item/status 枚举。

## 1. Response envelope

```json
{
  "id": "resp_...",
  "object": "response",
  "created_at": 0,
  "model": "gpt-5.6",
  "status": "completed",
  "output": [{
    "type": "message",
    "id": "msg_...",
    "role": "assistant",
    "status": "completed",
    "content": [{"type": "output_text", "text": "...", "annotations": []}]
  }],
  "output_text": "...",
  "usage": {"input_tokens": 0, "output_tokens": 0, "total_tokens": 0}
}
```

## 2. `output[]` 与 convenience view

`output[]` 是权威的有序结构化输出。它可包含 message、reasoning、client tool call 或 hosted-tool item。`output_text` 只是从 message
text 聚合出的方便读取属性，不能替代完整 item log。

客户端应把最终展示 text 与可 replay 的 `output[]` 分开保存。item identity、type、顺序、status、annotation 和 opaque continuation
data 都不能由 `output_text` 恢复。

Function/custom tool item 的闭环见 [Function tools](function-tools.md)。

## 3. Response status

常见状态包括 queued、in_progress、completed、incomplete、failed 与 cancelled：

- `completed`：完整 response lifecycle 结束，但 `output[]` 仍可能要求客户端执行 function call；
- `incomplete`：应检查 `incomplete_details`，不能把 partial output 当作完整答案；
- `failed`/`cancelled`：读取 error/status，不得伪造成 completed；
- 未知未来 status 应保留为未知枚举，并由显式 policy 处理。

## 4. Usage 与错误边界

Responses usage 使用 input/output/total token 命名；不能机械投影为 Chat prompt/completion usage，也不能虚构缺失数据。response-level
error、item status 与 transport failure 是不同层次。

## 5. 证据边界

- 本文不定义 SSE terminal event；见 [Responses SSE](streaming.md)；
- 单个 completed text response 不证明 reasoning、tool、hosted tool、incomplete、refusal 或 error shape；
- deterministic JSON fixture 不证明 resource storage、真实 Provider 或当前 SDK 行为。
