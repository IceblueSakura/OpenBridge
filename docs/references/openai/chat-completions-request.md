# OpenAI Chat Completions JSON 请求调研

## 来源、范围与快照

本文只记录 `POST /v1/chat/completions` 的 JSON request envelope、`messages[]` 顺序与文本消息语义。图片、文件和音频 part
分别由对应模态文档维护；工具、结构化输出和 SSE 也不在本文重复展开。

- 官方来源：[Create chat completion](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create)
- 协议复核日期：2026-08-03；本次结构整理未重新在线复核动态字段或 model capability。

## 1. HTTP 与最小 request

```http
POST /v1/chat/completions
Content-Type: application/json
Authorization: Bearer $OPENAI_API_KEY

{
  "model": "gpt-5.6",
  "messages": [
    {"role": "user", "content": "Explain this API."}
  ]
}
```

`model` 与有序 `messages[]` 构成核心输入。参数是否可用仍由具体 model/profile 决定；通用 JSON schema 接受某字段，不等于任意
model 都支持。

## 2. `messages[]` 顺序与 role

| role        | 主要用途                 | 必须保持的关系                                                |
|-------------|--------------------------|---------------------------------------------------------------|
| `developer` | 开发者指令               | 对较新模型可承担优先指令位置，不能随意降为 user text          |
| `system`    | 系统指令                 | 仍是独立协议 role，不能静默删除                               |
| `user`      | 终端用户输入与上下文     | string text 或 model/profile 允许的 typed content parts       |
| `assistant` | 历史模型输出             | 可含 text、refusal、tool calls 或 profile-specific output     |
| `tool`      | 客户端执行的工具结果     | 通过 `tool_call_id` 对应先前 assistant tool call              |
| `function`  | 旧 function-calling 形状 | 历史兼容面；新调用以 `tool` 与 `tool_call_id` 为主            |

message 序列表示“谁在何时提供了什么”。合并 message、按 role 重排、提前插入 tool result 或把 typed part 转成纯文本都会改变语义。

文本输入可使用 string 或 text content part。其他 content part 只在以下 owner 文档说明：

- [Chat 图片输入](images-chat-input.md)
- [Chat 文件输入](files-chat-input.md)
- [Chat 音频输入/输出](audio-chat-input-output.md)

## 3. 顶层字段分组

| 分组                 | 代表字段                                                                    | 边界                                                             |
|----------------------|-----------------------------------------------------------------------------|------------------------------------------------------------------|
| 生成预算             | `max_completion_tokens`                                                     | 与 Responses `max_output_tokens` 不是同一 wire 字段              |
| 采样与候选           | `temperature`、`top_p`、`seed`、`n`、`stop`、`logprobs`、`top_logprobs`     | 支持度依 model 而异                                              |
| Reasoning            | `reasoning_effort`                                                          | 与 Responses `reasoning.effort` 只能经显式转换                   |
| 工具                 | `tools`、`tool_choice`、`parallel_tool_calls`                               | 见 [Function tools](chat-completions-function-tools.md)                            |
| 结构化输出           | `response_format`                                                           | 见 [Structured output](chat-completions-structured-output.md)                      |
| Streaming            | `stream`、`stream_options`                                                  | 见 [Chat SSE](chat-completions-streaming.md)                                       |
| 媒体输出             | `modalities`、`audio`                                                       | 见 [Chat 音频输入/输出](audio-chat-input-output.md)            |
| 存储与运营控制       | `store`、`metadata`、`service_tier`、`user`、cache/safety identifier 等     | 必须按 Provider 与数据治理 contract 单独 gate                    |
| 专用能力             | `prediction`、`web_search_options`                                          | 不能降级为普通 prompt 或 client function tool                    |

## 4. 证据边界

- 本文不定义 non-streaming response、SSE chunk、工具闭环或结构化输出 schema；
- SDK request type 不能替代 HTTP/JSON wire 与 model-specific capability；
- 一个文本 request 成功不证明图片、文件、音频、工具、structured output 或 streaming 可用；
- Chat 仍受支持，但官方对新项目建议优先考察 Responses；这不等于 Chat 已失效。
