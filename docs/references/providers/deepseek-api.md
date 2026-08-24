# DeepSeek API 协议入口

- Last reverified：外部来源最后复核 2026-08-19；2026-08-24 仅整理本地文档，未刷新外部来源。
- Recheck trigger：Chat/Responses/Anthropic endpoint、SSE 终态、认证或 JSON mode 变化。

## 来源与范围

- [Chat Completions API](https://api-docs.deepseek.com/api/create-chat-completion)
- [Responses API](https://api-docs.deepseek.com/guides/responses_api/)
- [JSON Output](https://api-docs.deepseek.com/guides/json_mode/)
- [Function Calling](https://api-docs.deepseek.com/guides/function_calling/)
- [Thinking Mode](https://api-docs.deepseek.com/guides/thinking_mode)

本文只保留 endpoint、认证和协议终态，不复制逐模型能力矩阵、参数集合、context、价格或并发限制。

## 入口与认证

- OpenAI-compatible base URL 为 `https://api.deepseek.com`；Chat Completions 与 Responses 相对入口分别为 `/chat/completions` 和 `/responses`。
- Anthropic-compatible base URL 为 `https://api.deepseek.com/anthropic`。
- 官方示例使用 Bearer API key。

## 协议事实

- Responses streaming 使用 typed semantic SSE events，并以 `response.completed`、`response.incomplete` 或 `response.failed` 终止，不使用 Chat Completions 的 `[DONE]` sentinel。
- Chat JSON mode 使用 `response_format: {"type":"json_object"}`；官方要求 prompt 明确请求 JSON，并提醒仍可能出现空内容。
- Chat 与 Responses 的字段集合并不相同；未声明或忽略字段不能记作可控能力。

逐模型支持、reasoning 档位、tool 类型和当前发布状态应直接读取 DeepSeek 官方文档。OpenBridge 当前映射见[Model 与 Provider 映射](../../implementation-status/model-provider-mapping.md)。

## 证据边界

官方协议页不证明任一 API key、账户、模型或参数组合当前可用，也不证明未来版本、SDK、负载或长时间 streaming 行为。
