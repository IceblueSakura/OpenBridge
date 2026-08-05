# Xiaomi MiMo 协议入口快照（2026-08-02）

## 来源与范围

本文只记录 2026-08-02 时 Xiaomi MiMo 官方文档公开的 Chat/Responses endpoint、认证、模型和明确限制，不包含任何本地接入状态。

- [Chat Completions API](https://mimo.mi.com/docs/zh-CN/api/chat/openai-api)
- [Responses API](https://mimo.mi.com/docs/zh-CN/api/chat/responses)
- [Models list](https://mimo.mi.com/docs/zh-CN/api/model/list-models)

## 观察事实

- Chat Completions 请求地址为 `https://api.xiaomimimo.com/v1/chat/completions`。
- Responses 请求地址为 `https://api.xiaomimimo.com/v1/responses`。
- 两份文档允许 `api-key` 或 `Authorization: Bearer` 认证。
- Responses 文档明确不支持 `background` 与 `previous_response_id`。
- 模型列表包含 `mimo-v2.5-pro` 与 `mimo-v2.5`，另有不属于本文文本生成范围的 ASR/TTS 变体。

## 证据边界

endpoint 和字段说明不能单独证明 streaming、parallel tools、image、structured output 或 reasoning output 的完整 wire 行为。模型列表也不证明某个账户当前具备调用资格。

