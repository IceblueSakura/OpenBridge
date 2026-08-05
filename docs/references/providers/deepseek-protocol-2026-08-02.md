# DeepSeek 协议入口快照（2026-08-02）

## 来源与范围

本文只记录 2026-08-02 时 DeepSeek 官方文档公开的 endpoint、认证和模型命名，不包含任何本地接入状态。

- [Function calling guide](https://api-docs.deepseek.com/guides/function_calling/)
- [Chat Completions API](https://api-docs.deepseek.com/api/create-chat-completion)
- [Updates](https://api-docs.deepseek.com/updates/)
- [Pricing and model names](https://api-docs.deepseek.com/quick_start/pricing/)

## 观察事实

- OpenAI-compatible base URL 为 `https://api.deepseek.com`。
- Chat Completions 相对入口为 `/chat/completions`。
- 官方示例通过 Bearer API key 认证。
- 2026-08-02 快照中的 V4 模型名为 `deepseek-v4-pro` 与 `deepseek-v4-flash`。
- 旧 `deepseek-chat` / `deepseek-reasoner` 名称处在官方停用边界。
- 本次资料只确认 Chat Completions 入口；没有据此确认 Responses endpoint。

## 证据边界

官方协议页说明公开 wire contract，不证明任一具体 API key、账户、区域或模型当前可用，也不证明所有可选 Chat 字段、SSE/tool lifecycle 已经通过真实请求验证。

