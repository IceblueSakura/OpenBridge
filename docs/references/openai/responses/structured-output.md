# OpenAI Responses Structured output 调研

## 来源、范围与快照

本文只记录 Responses Create 的 response-level structured output request wire 与结果判定。

- 官方来源：[Structured Outputs](https://platform.openai.com/docs/guides/structured-outputs)、[Create a response](https://developers.openai.com/api/reference/resources/responses/methods/create)
- 协议复核日期：2026-08-03；本次结构整理未重新在线复核 JSON Schema 方言或 model 支持。

## 1. Request wire

```json
{
  "text": {
    "format": {
      "type": "json_schema",
      "name": "answer",
      "strict": true,
      "schema": {"type": "object"}
    }
  }
}
```

Responses 的 structured-output 位置是 `text.format`。它不能接收 Chat `response_format` wrapper 后只改顶层字段名便宣称等价。

## 2. Result 判定

consumer 必须检查 response status、incomplete details、refusal 与实际 output item。JSON parse 成功不代表 schema adherence；
`output_text` 也不能替代完整 `output[]` 状态。

## 3. 与 Chat 的边界

Chat wire 见 [Chat Structured output](../chat-completions/structured-output.md)。跨协议转换需要显式 wrapper 映射、schema capability
check、refusal/incomplete 处理与 streaming test。

## 4. 证据边界

- schema 出现在 API Reference 不证明所有 model 支持；
- 单个 completed JSON response 不证明 strict、refusal、incomplete 或 SSE 路径；
- SDK parse helper 不能替代 wire 与 schema adherence 验证。
