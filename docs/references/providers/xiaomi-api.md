# Xiaomi MiMo API 协议入口

- Last reverified：外部来源最后复核 2026-08-09；2026-08-24 仅整理本地文档，未刷新外部来源。
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

Models 目录可见性不证明某个 operation、参数、streaming 或媒体任务当前可用。具体模型能力和生命周期应直接读取 MiMo 官方文档；OpenBridge 当前映射见[Model 与 Provider 映射](../../implementation-status/model-provider-mapping.md)。

## 证据边界

本文不替代真实账号、错误、负载或长期运行验证。动态 endpoint 和 Provider 行为变化时需要重新读取官方资料。
