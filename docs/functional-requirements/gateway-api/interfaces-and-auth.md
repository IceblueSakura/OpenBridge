# 接口与认证

## 状态

本文是[网关 API 域](README.md)的接口与认证模块：定义可调用的 endpoint 集合、认证边界和 MCP 本地工具入口。
其他模块见[网关 API 域](README.md)导航。

## 1. 接口总览

| 接口                                                             | 功能要求                                                                                                              | 不包含的语义                                                                                 |
|------------------------------------------------------------------|-----------------------------------------------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------|
| `GET /healthz`                                                   | 提供不访问上游凭证的最小本地存活信息；不得泄露 route、Upstream Target 或 secret。                                     | Provider 健康探测、控制面或客户端管理。                                                      |
| `GET /v1/models`、`GET /v1/models/{model}`                       | 按[模型能力契约](../model-capability/README.md)返回严格的 OpenAI 标准四字段 list/retrieve。                            | 扩展能力、上游模型或部署信息。                                                               |
| `GET /openbridge/v1/models`、`GET /openbridge/v1/models/{model}` | 返回同一 Public Model 目录的模型事实和 Chat/Responses/Embeddings 固定能力契约。                                       | Provider/target/route、credential、健康、价格或动态发现。                                    |
| `POST /v1/chat/completions`                                      | 支持已声明能力范围内的 Chat JSON/SSE，并按[图片](../extended-capabilities/native-image.md)、[文件](../extended-capabilities/native-file.md)和[音频](../extended-capabilities/native-audio.md)需求提供 Native 媒体能力。 | 多模态 Bridge、未声明模型的 audio output、专用媒体/资源 API 或 hosted tool 的默认兼容承诺。 |
| `POST /v1/responses`                                             | 支持已声明能力范围内的 Responses JSON/SSE，并按[图片](../extended-capabilities/native-image.md)和[文件](../extended-capabilities/native-file.md)需求提供 Native 媒体输入。 | 多模态 Bridge、Responses audio/WebSocket、资源 retrieve/cancel/store/background/conversation API。 |
| `POST /v1/embeddings`                                            | 支持独立 Embedding Public Model 的 OpenAI-compatible JSON 请求/响应。                                                 | streaming、向量转换/存储/检索，或无等价证明的跨模型 fallback。                               |
| `POST /mcp`                                                      | 提供 MCP dual-era Streamable HTTP 本地入口：`2026-07-28` 无状态 discovery、静态 `hello` 目录与无副作用调用，同时接受 legacy `initialize` 握手客户端。 | 动态工具、Provider Bridge、外部 side effect、资源、prompt 或常驻通知 stream。 |

业务 endpoint 必须使用用户表分配的静态 Bearer API Key。用户表只在启动时读取，不提供在线 key issuance、scope、即时撤销、配额或
billing identity；变更需要重启。认证失败与未知/不支持 endpoint 必须在进入路由或上游调用前结束，且不泄露配置细节。

## 2. MCP 本地工具入口

- MCP endpoint 与 Chat/Responses 中的 function-tool wire 转发相互独立。它只提供显式注册的本地工具，不把 Public Model、Provider、
  Target、Route 或上游 credential 暴露为 MCP tool。
- endpoint 只接受带现有下游 Bearer token 的 `POST /mcp`。协议协商为 dual-era：`2026-07-28` 客户端走无状态路径，旧版客户端通过
  legacy `initialize`/`initialized` handshake 建立会话。`server/discover` 必须声明 `tools` capability 并通告支持的协议版本列表；
  `tools/list` 必须按确定性顺序返回唯一的 `hello`，其 closed `inputSchema` 只接受一个必需字符串 `name`。
- `tools/call` 只执行 `hello`：有效调用返回一个文本 content block `Hi, {name}!`，不读取配置、registry、文件、网络或 Provider。
  无效 `hello` argument 返回 `isError: true` 的工具结果；未知工具返回 JSON-RPC `-32602` protocol error。
- 无状态（`2026-07-28`）请求必须携带 `application/json` body、同时接受 JSON/SSE 的 `Accept`、`MCP-Protocol-Version` 与
  `Mcp-Method` header，并与 JSON-RPC body 中的 method、protocol version 和 per-request client capabilities 一致；`tools/call`
  还必须携带与 body tool name 一致的 `Mcp-Name`。缺失、畸形或不一致的 metadata 必须在任何工具执行前以稳定 JSON-RPC error 失败。
- 当前 endpoint 不接受任何 `Origin` header。带 Origin 的请求必须以 `403` 失败；无 Origin 的本地客户端仍受 loopback listener、
  Bearer 认证、全局请求体上限、请求 ID、敏感 header 与终态观测保护。
- 该 MCP 服务由官方 `rmcp` SDK 提供（`StreamableHttpService`），其自身执行 legacy session 管理（`Mcp-Session-Id`、GET SSE
  stream、DELETE session lifecycle）与版本协商；未认证请求一律在进入该服务前以 `401` 结束。协议无关的未实现 RPC method 由 SDK
  返回 JSON-RPC `-32601`。

## 关联文档

- [网关 API 域导航](README.md)
- [Public Model 与模型能力契约](../model-capability/README.md)
- [配置与凭证](../configuration-credentials/README.md)
- [当前实现总览](../../implementation-status/current-implementation.md)
