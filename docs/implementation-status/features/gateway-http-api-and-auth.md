# 功能：HTTP 网关接口与下游认证

## 当前行为

- 未认证接口为 `GET /healthz`、`GET /openapi.yaml`、`GET /swagger-ui` 与 `GET /swagger-ui/`。
- Bearer 保护接口包括标准/扩展 Models、`POST /v1/chat/completions`、`POST /v1/responses`、
  `POST /v1/embeddings`，以及 MCP 的 `POST /mcp` 与 legacy session `GET/DELETE /mcp`。
- 认证、请求 ID、body budget、敏感 header 标记和 tracing middleware 在业务 handler 前执行；失败认证返回 `401` 与
  `WWW-Authenticate: Bearer`。
- MCP 支持 `2026-07-28` stateless discovery，以及 legacy initialize/session/SSE/delete lifecycle；两者共享静态 `hello` 工具目录与
  `tools/call`。Origin、media type、Accept、mirror header、metadata 和 method 在独立 transport 边界校验。`hello` 不访问
  registry、credential 或 Provider。
- Models 只输出下游安全事实；Provider、Target、Route、upstream model、endpoint、credential、健康与价格不进入 DTO。
- 旧 `/openbridge/v1/metrics*` 路径未注册；metrics 只通过 startup-owned OTLP/HTTP exporter 输出。

## 所有权

Router/handler 位于 [`src/ingress/`](../../../src/ingress/)，MCP 位于 [`src/mcp/`](../../../src/mcp/)，内置 OpenAPI/Swagger
资源由 ingress 提供。当前没有在线用户管理、动态路由控制面、GUI 或请求级上游选择。

## 确定性证据

- `tests/ingress_contract.rs`：公开/受保护路由、body/JSON、旧 metrics 404 与 OpenAPI 资源。
- `tests/downstream_auth_contract.rs`：Bearer、失败响应和敏感信息边界。
- `tests/mcp_contract.rs`、`tests/mcp_dual_era.rs`：stateless/legacy lifecycle、discovery、工具目录/调用、认证、Origin、header、参数和 method 错误。
- `tests/example_config.rs`：随附 Bootstrap profile 可解析并编译。

## 未证明范围

进程内契约不证明外部 MCP/OpenAI SDK、浏览器、真实网络部署、反向代理、Provider、工具安全、负载或长期运行。

## 相关文档

- [网关 API 需求](../../functional-requirements/gateway-api/README.md)
- [启动配置与凭证](startup-configuration-and-credentials.md)
- [Models 与能力预检](models-api-and-capability-preflight.md)
- [当前代码架构](../current-architecture.md)
