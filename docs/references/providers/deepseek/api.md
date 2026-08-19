# DeepSeek API 协议入口调研

## 来源与范围

本文记录 2026-08-19 时 DeepSeek 官方文档公开的 endpoint、认证与 wire 约束，不包含本地接入状态。模型目录与定价见 [models.md](models.md)。

- [Responses API guide](https://api-docs.deepseek.com/guides/responses_api/)
- [JSON Output guide](https://api-docs.deepseek.com/guides/json_mode/)
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

- `deepseek-v4-flash` 与 `deepseek-v4-pro` 都被当前 Responses API Reference、兼容性指南和 2026-08-13 更新日志列为支持模型；仍称
  Pro 不支持的 Codex 集成页是过时材料。
- Responses 支持文本 input/message、instructions、streaming、`max_output_tokens`、temperature/top-p、reasoning、
  `top_logprobs`、`user`、function calling 与 server-side web search；图片与文件输入、`store`、`previous_response_id`、
  `conversation`、`background`、`include`、`prompt_cache_key`、`stop` 和 `logprobs` 不受支持或被忽略。
- streaming 使用 typed semantic SSE events，以 `response.completed`、`response.incomplete` 或 `response.failed` 终止，不使用 Chat Completions 的 `[DONE]` sentinel。

### JSON Output

- 官方模型矩阵为 `deepseek-v4-pro` 与 `deepseek-v4-flash` 都标记 JSON Output；Chat 通过
  `response_format: {"type":"json_object"}` 开启。
- system 或 user prompt 必须明确包含 `json`，并给出期望 JSON 字段或示例；输出 token 上限应足以容纳完整 JSON。官方同时提示该
  模式仍可能偶发空内容，可通过调整 prompt 缓解。
- V4 Responses 文档将 `text.format` 标记为完全支持；OpenAI-compatible Responses 对应 `text`、`json_object` 和
  `json_schema`。Chat 仍只公开 `text`/`json_object` 响应格式。

### 其他协议面

- V4 thinking mode 默认启用；Chat 与 Responses schema 都列出 `none/auto/required/named` function tool choice。Responses 还支持
  server-side web search 和 Codex `apply_patch` 特例，但不能泛化为任意 hosted/custom tool。
- Chat 顶层 schema 包含 `max_tokens`、`stop`、`stream_options`、sampling、function tools、`logprobs`/`top_logprobs` 与
  `user_id`。penalties 已废弃且无效，thinking 开启时 sampling 参数也无效。
- Pro `low` 的官方资料仍冲突：2026-08-13 更新日志与 thinking guide 声称支持，Chat API Reference 仍称按 `high` 处理。
- 官方文档还列出 web search、custom `apply_patch`、`text.format`、Chat Prefix Completion（Beta）、FIM Completion（Beta，仅非思考模式）等能力；这些目录或指南事实不能自动证明任一 consumer/gateway 已公开相同能力，被上游忽略的字段也不能记作可控能力。
- 模型特性矩阵（JSON Output、Tool Calls、Anthropic API、Chat Prefix、FIM 的逐模型支持）见 [models.md](models.md)。

## 证据边界

官方协议页说明公开 wire contract，不证明任一具体 API key、账户、区域或模型当前可用，也不证明其他账户、未来行为、SDK 版本兼容性
或长时间 streaming。模型/API 发布状态、Responses 支持范围或字段语义变化时，需要重新固定官方页面并执行独立请求验证。
