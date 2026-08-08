# DeepSeek 协议入口快照（2026-08-08）

## 来源与范围

本文只记录 2026-08-08 时 DeepSeek 官方文档公开的 endpoint、模型范围与 Responses wire 约束，不包含本地接入状态。

- [Responses API guide](https://api-docs.deepseek.com/guides/responses_api/)
- [Function calling guide](https://api-docs.deepseek.com/guides/function_calling/)
- [Chat Completions API](https://api-docs.deepseek.com/api/create-chat-completion)
- [Updates](https://api-docs.deepseek.com/updates/)
- [Pricing and model names](https://api-docs.deepseek.com/quick_start/pricing/)

## 观察事实

- OpenAI-compatible base URL 为 `https://api.deepseek.com`；Chat Completions 相对入口为 `/chat/completions`，Responses 相对入口为
  `/responses`。
- 官方示例通过 Bearer API key 认证，并通过 OpenAI SDK 的 `client.responses.create` 调用 Responses。
- 当前只有 `deepseek-v4-flash` 声明支持 Responses；`deepseek-v4-pro` 仍不支持该入口。
- Responses 支持文本 input/message、instructions、streaming、标准输出 token/temperature/top-p 参数、reasoning effort 与 function
  calling；图片与文件输入、`store`、`previous_response_id`、`conversation` 和 `background` 不受支持。
- 官方文档还列出 web search、custom `apply_patch`、`text.format` 等能力，但这些事实不能自动扩大 OpenBridge 当前公开能力；
  被上游忽略的字段也不能记作可控能力。
- streaming 使用 typed semantic SSE events，以 `response.completed`、`response.incomplete` 或 `response.failed` 终止，不使用 Chat
  Completions 的 `[DONE]` sentinel。

## 证据边界

官方协议页说明公开 wire contract，不证明任一具体 API key、账户、区域或模型当前可用。本文没有执行真实 DeepSeek 请求，因而不证明
实际响应内容、reasoning 形态、错误分类、SDK 版本兼容性或长时间 streaming 行为；这些结论必须由独立的真实 Provider 验收建立。
