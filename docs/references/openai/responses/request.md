# OpenAI Responses Create JSON 请求调研

## 来源、范围与快照

本文只记录 `POST /v1/responses` create operation 的 JSON request envelope、文本 input/item 模型和 reasoning request 字段。
图片、文件、工具、结构化输出、streaming 与 server-side state 各由独立文档维护。

- 官方来源：[Create a response](https://developers.openai.com/api/reference/resources/responses/methods/create)、[Reasoning models](https://developers.openai.com/api/docs/guides/reasoning)
- 协议复核日期：2026-08-03；本次结构整理未重新在线复核 beta 字段或 model capability。

## 1. HTTP 与最小 request

```http
POST /v1/responses
Content-Type: application/json
Authorization: Bearer $OPENAI_API_KEY

{
  "model": "gpt-5.6",
  "input": "Explain the Responses API."
}
```

字符串 `input` 表示 user text input。复杂请求使用有序异构 `input[]`，其中 message 只是 item union 的一种成员。

## 2. Ordered item log

`input[]` 可包含 role message、先前 assistant output、reasoning、function/custom tool call 及其 output，以及 model/profile 允许的其他
item。consumer 必须按 `type` 解析并保持 item 顺序、identity、内容与状态；不能只读取 `role` 或把所有 item 渲染成文本。

message role 可包括 `user`、`assistant`、`system`、`developer`。文本 part 使用 `input_text`；其他模态分别见：

- [Responses 图片输入](../images/responses-input.md)
- [Responses 文件输入](../files/responses-input.md)

## 3. 关键字段归属

| 分组                 | 代表字段                                                               | Owner 文档或边界                                             |
|----------------------|------------------------------------------------------------------------|--------------------------------------------------------------|
| 指令与输入           | `instructions`、`input`                                                | 本文                                                         |
| Reasoning            | `reasoning`                                                             | 本文                                                         |
| 工具                 | `tools`、`tool_choice`、`parallel_tool_calls`、`max_tool_calls`        | [Function tools](function-tools.md)                           |
| 结构化输出           | `text.format`                                                           | [Structured output](structured-output.md)                     |
| Streaming            | `stream`                                                                | [Responses SSE](streaming.md)                                 |
| Continuation/state   | `conversation`、`previous_response_id`、`store`                         | [State ownership](state.md)                                   |
| Background/resources | `background`                                                            | [Resource lifecycle](resource-lifecycle.md)                   |
| Context/output limit | `truncation`、`context_management`、`max_output_tokens`                 | 需按 model/profile gate                                      |
| 输出附加项           | `include`                                                               | 枚举逐项 gate；不能从一项支持推断其他项                       |
| 运营控制             | `metadata`、prompt/cache/service tier/safety identifier 等             | 需按 Provider 与数据治理 contract gate                       |

## 4. `reasoning` 的 wire 位置

Responses 使用顶层 `reasoning` 配置对象，例如 `reasoning: {"effort": "low"}`。当前快照未把顶层 `reasoning_effort` 定义为标准
Responses Create 字段；该名称属于 Chat 或 Provider 私有兼容面时，不能反向写成 Responses 标准。

`output[]` 中的 `type: "reasoning"` 是输出 item，不是 request 配置对象。`reasoning.encrypted_content` 是 opaque continuation
data，不应作为普通明文 reasoning 处理。

## 5. 证据边界

- 通用 request schema 不证明具体 model 支持全部 item、reasoning 或 include 值；
- 本文不定义 non-stream response、SSE、tool loop 或 server resource operations；
- 字符串 input 成功不证明图片、文件、工具、state 或 background 可用。
