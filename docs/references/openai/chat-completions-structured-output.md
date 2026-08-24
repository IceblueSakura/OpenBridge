# OpenAI Chat Completions Structured output 调研

## 来源、范围与快照

本文只记录 Chat Completions response-level structured output 的 request wire 与结果判定。

- 官方来源：[Structured Outputs](https://platform.openai.com/docs/guides/structured-outputs)、[Create chat completion](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create)
- 协议复核日期：2026-08-03；本次结构整理未重新在线复核 JSON Schema 方言或 model 支持。

## 1. Request wire

```json
{
  "response_format": {
    "type": "json_schema",
    "json_schema": {
      "name": "answer",
      "strict": true,
      "schema": {"type": "object"}
    }
  }
}
```

`json_object` 与 JSON Schema Structured Outputs 不是同一保证：前者关注 valid JSON，后者在 model/profile 支持时承诺 schema
adherence。

## 2. Result 判定

客户端仍需检查 response terminal、refusal、截断/content filter 与 message content。JSON parse 成功不代表 request schema 已满足；
只有相应 structured-output contract 明确成立时才能宣称 schema adherence。

## 3. 与 Responses 的边界

Chat 使用 `response_format` 与 `json_schema` wrapper；Responses 使用 `text.format`。二者共享 structured-output 意图，但不是字节级
同构对象。转换需要显式字段映射、schema compatibility test 与目标 model capability gate。

Responses wire 见 [Responses Structured output](responses-structured-output.md)。

## 4. 证据边界

- 通用 OpenAPI schema 接受 `response_format` 不证明目标 model 支持；
- 一个成功 JSON object 不证明 strict schema、refusal、incomplete 或 streaming 路径；
- SDK parse helper 不能替代 HTTP response 与 schema adherence 验证。
