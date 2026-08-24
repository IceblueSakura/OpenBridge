# OpenRouter API 调研

- Last reverified：官方 API 资料与 live wire 原始复核为 2026-08-02 至 2026-08-09；2026-08-24 仅整理本地文档，未刷新外部来源。
- Recheck trigger：Responses beta、Provider routing、Models/endpoint schema 或数据政策变化。

## 来源与范围

- [Chat Completions API](https://openrouter.ai/docs/api/api-reference/chat/send-chat-completion-request)
- [Responses API](https://openrouter.ai/docs/api/reference/responses/overview)
- [Models API](https://openrouter.ai/docs/api/api-reference/models/get-models)
- [Provider Routing](https://openrouter.ai/docs/guides/routing/provider-selection)

本文只记录入口、认证、Provider routing 与固定 wire 观察，不保存模型字段、能力表、价格或 Models 快照。

## API 与认证

- API base 为 `https://openrouter.ai/api/v1`。
- Chat Completions、Responses 和 Models 相对 path 分别为 `/chat/completions`、`/responses`、`/models`。
- API key 使用 `Authorization: Bearer ***`。
- `HTTP-Referer` 与 `X-Title` 是可选 attribution/routing header，不是认证字段。
- Responses surface 无状态；官方资料将 `store: true` 与非空 `previous_response_id` 列为不支持。

## Provider routing

Models 目录、用户过滤视图和单模型 endpoint 列表是不同资源；目录可见不证明某个 endpoint、账户或参数组合当前可用。
`provider.require_parameters` 默认为 `false`；设为 `true` 才要求候选 Provider 声明支持请求中的全部参数。具体模型与 endpoint 能力应在采用时重新读取 OpenRouter 官方资源，不在本文复制。

## 固定 wire 观察

2026-08-02 的一次 Responses streaming 请求得到 data-only SSE：终态 JSON 顶层 `type` 为 `response.completed`，嵌套 `response.status` 为 `completed`，随后另有 `[DONE]`。该结果只证明当时账户、模型、网络和 payload，不证明其他 endpoint、模型或未来版本。

## 证据边界

统一 API surface 不表示所有模型共享相同能力、数据政策、配额或 SLA。模型能力以 OpenRouter 当前官方页面和 endpoint 详情为准；OpenBridge 当前映射见[Model 与 Provider 映射](../../implementation-status/model-provider-mapping.md)。
