# Xiaomi MiMo API 协议入口

- Last reverified：2026-08-31；刷新 Chat、Responses 与 Models 官方页面。
- Recheck trigger：origin、认证、Chat/Responses/Models endpoint 或媒体协议变化。

## 来源与范围

- [Chat Completions API](https://mimo.mi.com/docs/zh-CN/api/chat/openai-api)
- [Responses API](https://mimo.mi.com/docs/zh-CN/api/chat/responses)
- [Models API](https://mimo.mi.com/docs/zh-CN/api/model/list-models)
- [图片协议与固定 wire 观察](xiaomi-image.md)
- [音频协议与固定 wire 观察](xiaomi-audio.md)

本文只记录公共 origin、认证和 endpoint，不复制逐模型能力、参数或下线列表。

## 入口与认证

- API origin 为 `https://api.xiaomimimo.com`。
- Chat Completions、Responses 和 Models 相对入口分别为 `/v1/chat/completions`、`/v1/responses` 和 `/v1/models`。
- 认证支持 `api-key: ***` 或 `Authorization: Bearer ***`，二选一。
- 官方 Responses 文档将 `background` 与 `previous_response_id` 列为不支持。
- [Chat 结构化输出专页](https://mimo.mi.com/docs/en-US/quick-start/usage-guide/text-generation/structured-output)明确为 MiMo-V2.5/Pro 提供 `json_object`；Chat API reference 同时只列 `text`，两者存在官方文档冲突，因此 executable caps 只保留专页明确的 Chat JSON Object，不提升 JSON Schema。
- Responses 当前只列 `text` format，也未声明 `prompt_cache_key` 或 `include`；这些 Responses 字段不作为 executable caps 公开。

Models 目录可见性不证明某个 operation、参数、streaming 或媒体任务当前可用。具体模型能力和生命周期应直接读取 MiMo 官方文档；OpenBridge 当前映射见[Model 与 Provider 映射](../../implementation-status/model-provider-mapping.md)。

## 证据边界

本文不替代真实账号、错误、负载或长期运行验证。动态 endpoint 和 Provider 行为变化时需要重新读取官方资料。
