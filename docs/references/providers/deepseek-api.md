# DeepSeek API 协议入口

- Last reverified：2026-08-31。
- Recheck trigger：Chat/Responses/Anthropic endpoint、SSE 终态、认证、JSON mode、Vision 来源/格式/detail/limit 或 Files API 变化。

## 来源与范围

- [Chat Completions API](https://api-docs.deepseek.com/api/create-chat-completion)
- [Responses API](https://api-docs.deepseek.com/guides/responses_api/)
- [JSON Output](https://api-docs.deepseek.com/guides/json_mode/)
- [Function Calling](https://api-docs.deepseek.com/guides/function_calling/)
- [Tool Calls / strict mode](https://api-docs.deepseek.com/guides/tool_calls/)
- [Thinking Mode](https://api-docs.deepseek.com/guides/thinking_mode)
- [Vision](https://api-docs.deepseek.com/guides/vision)

本文只保留 endpoint、认证和协议终态，不复制逐模型能力矩阵、参数集合、context、价格或并发限制。

## 入口与认证

- OpenAI-compatible base URL 为 `https://api.deepseek.com`；Chat Completions 与 Responses 相对入口分别为 `/chat/completions` 和 `/responses`。
- Anthropic-compatible base URL 为 `https://api.deepseek.com/anthropic`。
- 官方示例使用 Bearer API key。

## 协议事实

- Responses streaming 使用 typed semantic SSE events，并以 `response.completed`、`response.incomplete` 或 `response.failed` 终止，不使用 Chat Completions 的 `[DONE]` sentinel。
- Chat JSON mode 使用 `response_format: {"type":"json_object"}`；官方要求 prompt 明确请求 JSON，并提醒仍可能出现空内容。
- Chat 与 Responses 的字段集合并不相同；未声明或忽略字段不能记作可控能力。
- Function-tool strict mode 只在 `/beta` endpoint 有官方保证；当前普通 `https://api.deepseek.com` Target 不公开 function strict schema。Responses output JSON Schema 与 function-tool strict 是不同能力。
- V4 Pro 与 Flash 的官方 reasoning 档位包含 `low/high/max`；Vision Responses 另明确支持 `max_output_tokens` 与 structured output 参数。

逐模型支持、reasoning 档位、tool 类型和当前发布状态应直接读取 DeepSeek 官方文档。OpenBridge 当前映射见[Model 与 Provider 映射](../../implementation-status/model-provider-mapping.md)。

## 证据边界

官方协议页不证明任一 API key、账户、模型或参数组合当前可用，也不证明未来版本、SDK、负载或长时间 streaming 行为。
