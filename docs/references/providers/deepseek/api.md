# DeepSeek API 协议入口调研

## 来源与范围

本文记录 2026-08-09 时 DeepSeek 官方文档公开的 endpoint、认证与 wire 约束，不包含本地接入状态。模型目录与定价见 [models.md](models.md)。

- [Responses API guide](https://api-docs.deepseek.com/guides/responses_api/)
- [Function calling guide](https://api-docs.deepseek.com/guides/function_calling/)
- [Thinking mode](https://api-docs.deepseek.com/guides/thinking_mode)
- [Oh My Pi integration](https://api-docs.deepseek.com/quick_start/agent_integrations/oh_my_pi/)
- [Chat Completions API](https://api-docs.deepseek.com/api/create-chat-completion)
- [Models & Pricing](https://api-docs.deepseek.com/quick_start/pricing/)
- [Updates](https://api-docs.deepseek.com/updates/)

## 观察事实

### 入口与认证

- OpenAI-compatible base URL 为 `https://api.deepseek.com`；Chat Completions 相对入口为 `/chat/completions`，Responses 相对入口为 `/responses`。
- Anthropic 兼容 base URL 为 `https://api.deepseek.com/anthropic`（官方 Models & Pricing 页声明）。
- 官方示例通过 Bearer API key 认证，并通过 OpenAI SDK 的 `client.responses.create` 调用 Responses。

### Responses 约束

- 当前只有 `deepseek-v4-flash` 声明支持 Responses；`deepseek-v4-pro` 仍不支持该入口（官方页标注预计 2026 年 8 月初支持）。
- Responses 支持文本 input/message、instructions、streaming、标准输出 token/temperature/top-p 参数、reasoning effort 与 function calling；图片与文件输入、`store`、`previous_response_id`、`conversation` 和 `background` 不受支持。
- streaming 使用 typed semantic SSE events，以 `response.completed`、`response.incomplete` 或 `response.failed` 终止，不使用 Chat Completions 的 `[DONE]` sentinel。

### 其他协议面

- V4 thinking mode 默认启用并支持模型自主产生 tool call；官方 Oh My Pi 兼容页同时要求 `supportsToolChoice: false`，明确提示 V4
  thinking mode 会拒绝强制 `tool_choice` 参数。通用 Chat Completion schema 虽列出 `none/auto/required/named`，但不能据此推导
  Responses 默认 thinking 路径对四档均可执行。
- 官方文档还列出 web search、custom `apply_patch`、`text.format`、Chat Prefix Completion（Beta）、FIM Completion（Beta，仅非思考模式）等能力，但这些事实不能自动扩大 OpenBridge 当前公开能力；被上游忽略的字段也不能记作可控能力。
- 模型特性矩阵（JSON Output、Tool Calls、Anthropic API、Chat Prefix、FIM 的逐模型支持）见 [models.md](models.md)。

## 证据边界

官方协议页说明公开 wire contract，不证明任一具体 API key、账户、区域或模型当前可用。本文没有执行真实 DeepSeek 请求，因而不证明实际响应内容、reasoning 形态、错误分类、SDK 版本兼容性或长时间 streaming 行为；这些结论必须由独立的真实 Provider 验收建立。
