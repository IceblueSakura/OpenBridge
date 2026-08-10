# 功能：HTTP 网关接口与下游认证

## 状态

**已完成（当前 checkout）。** 本页只记录网关已经提供的客户端可见 HTTP 边界，不把内部 Provider、Target、Route 或 credential
拓扑暴露为下游契约。

## 已完成内容

- 提供未认证的 `GET /healthz`、`GET /openapi.yaml`、`GET /swagger-ui` 和 `GET /swagger-ui/`。
- 提供 Bearer 保护的 `GET /v1/models`、`GET /v1/models/{model}`、扩展 Models 接口、
  `POST /v1/chat/completions`、`POST /v1/responses` 和 `POST /v1/embeddings`。
- 提供 Bearer 保护的 `POST /mcp`，实现 MCP `2026-07-28` `server/discover`、确定性的 `hello` 工具目录和
  `tools/call`；`hello` 只格式化字符串，不建立 session，也不访问 Provider。
- 旧 `GET /openbridge/v1/metrics` 与 `GET /openbridge/v1/metrics/providers` 未注册并返回 `404`；metrics 是独立的
  startup-owned OTLP/HTTP 出站 signal，不属于下游 API 或 OpenAPI schema。
- 在进入业务 handler 前执行 Bearer 认证、请求体上限、请求 ID、敏感 `Authorization` header 标记和 tracing middleware。
- 认证失败统一返回 `401` 与 `WWW-Authenticate: Bearer`；模型列表和业务请求共享同一认证边界。
- MCP Route 在认证前拒绝所有带 `Origin` 的请求，并校验 JSON media type、Accept、protocol/method mirror header 和
  per-request metadata；只支持 POST，已认证 GET/DELETE 返回 `405`。
- `/v1/models` 只返回标准公共模型字段；扩展 Models 只返回下游可用的模型事实与 operation 能力，不返回 Provider、Target、Route、
  upstream model、endpoint、credential、健康状态或价格信息。

## 实现边界

- Router 组装位于 [`src/ingress/router.rs`](../../../src/ingress/router.rs)，业务 handler 位于
  [`src/ingress/handlers.rs`](../../../src/ingress/handlers.rs)。
- MCP crate-level facade 位于 [`src/mcp/mod.rs`](../../../src/mcp/mod.rs)，transport validation/dispatch 位于
  [`src/mcp/transport.rs`](../../../src/mcp/transport.rs)，确定性目录/分派位于
  [`src/mcp/tools/mod.rs`](../../../src/mcp/tools/mod.rs)，`hello` schema、argument validation 与执行位于
  [`src/mcp/tools/hello.rs`](../../../src/mcp/tools/hello.rs)；这些模块不进入 registry、pipeline、Provider adapter 或 upstream transport。
- OpenTelemetry instruments 与 exporter 位于 [`src/observability/`](../../../src/observability)，不在 HTTP handler 中提供查询或
  第二套聚合逻辑。
- OpenAPI 与 Swagger UI 是本地服务内置资源，不代表已通过外部 SDK 或浏览器客户端完成验收。
- 接口只面向静态下游用户表和固定 Public Model；没有在线用户管理、GUI、动态路由控制面或请求级上游选择。

## 验证证据

- [`tests/ingress_contract.rs`](../../../tests/ingress_contract.rs) 覆盖公开/受保护路由、请求边界和 handler 前置行为。
- [`tests/downstream_auth_contract.rs`](../../../tests/downstream_auth_contract.rs) 覆盖 Bearer 认证、失败响应和敏感信息边界。
- [`tests/ingress_contract.rs`](../../../tests/ingress_contract.rs) 还覆盖旧 metrics path 的 `404` 与 OpenAPI schema 移除。
- [`tests/mcp_contract.rs`](../../../tests/mcp_contract.rs) 的 3 个测试覆盖 discovery、`hello` 目录/成功调用/无效参数、Bearer、Origin、
  header mismatch、未知工具和非 POST method。
- [`tests/example_config.rs`](../../../tests/example_config.rs) 覆盖示例配置可解析性。

2026-08-10 为 `hello` 工具切片实际执行：

- `cargo test --locked --test mcp_contract`：3 passed；
- `cargo fmt -- --check`；
- `cargo test --locked`：第二次完整运行通过；第一次运行只有 `otlp_trace_contract` 的 span 数量断言出现一次性失败，随后该测试
  单独复跑 2 passed，未修改遥测代码；
- `cargo clippy --locked -- -D warnings`；
- `git diff --check`。

测试与 clippy 使用隔离的临时 `CARGO_TARGET_DIR`。未运行外部 MCP SDK/Client、真实网络部署、浏览器、反向代理、Provider、负载或
长期运行验收。

这些是进程内确定性契约测试，不等同于真实网络部署、外部 MCP SDK/Client、浏览器、生产反向代理或工具安全验收。

## 相关文档

- [功能需求：网关 API 与客户端兼容](../../functional-requirements/gateway-api-compatibility.md)
- [OpenAI API 端点采用与 fake 合同测试调研](../../references/openai/endpoint-adoption-and-fake-testing.md)
- [启动配置与凭证边界](startup-configuration-and-credentials.md)
- [模型信息与能力预检](models-api-and-capability-preflight.md)
- [当前代码架构](../current-architecture.md)
