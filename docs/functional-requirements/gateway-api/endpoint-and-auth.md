本文定义下游可调用的 endpoint 集合与认证边界。MCP 的协议生命周期由
[MCP 本地服务](mcp.md)单独拥有。

## 1. Endpoint 总览

| 接口 | 功能要求 | 不包含的语义 |
|---|---|---|
| `GET /healthz` | 提供不访问上游 credential 的最小本地存活信息，不泄露 Route、Target 或 secret。 | Provider 健康 probe、控制面或客户端管理。 |
| `GET /v1/models`、`GET /v1/models/{model}` | 按[模型能力契约](../model-capability.md)返回严格 OpenAI 四字段 list/retrieve。 | 扩展能力、上游模型或部署信息。 |
| `GET /openbridge/v1/models`、`GET /openbridge/v1/models/{model}` | 返回同一 Public Model 目录的模型事实和固定接口契约。 | Provider/Target/Route、credential、健康、价格或动态发现。 |
| `POST /v1/chat/completions` | 在固定 Chat interface 内提供 JSON/SSE 与已声明的 Native 媒体能力。 | 多模态 Bridge、未声明 audio output、专用资源 API 或 hosted tool 默认兼容。 |
| `POST /v1/responses` | 在固定 Responses interface 内提供 JSON/SSE 与已声明的 Native 媒体输入。 | 多模态 Bridge、Responses audio/WebSocket、response 资源与 background API。 |
| `POST /v1/embeddings` | 提供独立 Embedding Public Model 的有界 JSON 请求/响应。 | streaming、向量转换/存储/检索或无 identity 证明的 fallback。 |
| `/mcp` | 按 [MCP 本地服务](mcp.md)提供 stateless 与 legacy session 两种 Streamable HTTP lifecycle。 | 动态 tool、Provider Bridge、外部 side effect、resource 或 prompt。 |

## 2. 认证边界

- Models、generation、Embeddings 与 MCP endpoint 使用私有用户表分配的静态 Bearer API Key。
- 用户表只在启动时读取；不提供在线 key issuance、scope、即时撤销、配额或 billing identity，变更需要重启。
- 认证失败必须在模型查询、JSON-RPC dispatch、Route planning 或 Provider egress 前结束。
- 未认证错误不得泄露用户、registry、credential、endpoint、Route 或 MCP session 是否存在。
- `GET /healthz`、OpenAPI 与 Swagger UI 是公开资源，但只能暴露其各自的静态最小信息。
