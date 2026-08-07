# 功能：HTTP 网关接口与下游认证

## 状态

**已完成（当前 checkout）。** 本页只记录网关已经提供的客户端可见 HTTP 边界，不把内部 Provider、Target、Route 或 credential
拓扑暴露为下游契约。

## 已完成内容

- 提供未认证的 `GET /healthz`、`GET /openapi.yaml`、`GET /swagger-ui` 和 `GET /swagger-ui/`。
- 提供 Bearer 保护的 `GET /v1/models`、`GET /v1/models/{model}`、扩展 Models 接口、
  `POST /v1/chat/completions`、`POST /v1/responses`、`POST /v1/embeddings` 以及当前运行指标读取接口。
- `GET /openbridge/v1/metrics` 返回进程级 `GatewayMetricsSnapshot`，`GET /openbridge/v1/metrics/providers` 返回按受信编译维度排序的
  `ProviderMetricSnapshot` 列表；两次读取都只做认证和快照序列化，不创建请求观测，也不改变快照。
- 在进入业务 handler 前执行 Bearer 认证、请求体上限、请求 ID、敏感 `Authorization` header 标记和 tracing middleware。
- 认证失败统一返回 `401` 与 `WWW-Authenticate: Bearer`；模型列表和业务请求共享同一认证边界。
- `/v1/models` 只返回标准公共模型字段；扩展 Models 只返回下游可用的模型事实与 operation 能力，不返回 Provider、Target、Route、
  upstream model、endpoint、credential、健康状态或价格信息。

## 实现边界

- Router 组装位于 [`src/ingress/router.rs`](../../../src/ingress/router.rs)，业务 handler 位于
  [`src/ingress/handlers.rs`](../../../src/ingress/handlers.rs)。
- 指标快照由 [`src/observability/metrics.rs`](../../../src/observability/metrics.rs) 维护，HTTP 读取仍复用现有内存状态，不引入持久化或
  第二套聚合逻辑。
- OpenAPI 与 Swagger UI 是本地服务内置资源，不代表已通过外部 SDK 或浏览器客户端完成验收。
- 接口只面向静态下游用户表和固定 Public Model；没有在线用户管理、GUI、动态路由控制面或请求级上游选择。

## 验证证据

- [`tests/ingress_contract.rs`](../../../tests/ingress_contract.rs) 覆盖公开/受保护路由、请求边界和 handler 前置行为。
- [`tests/downstream_auth_contract.rs`](../../../tests/downstream_auth_contract.rs) 覆盖 Bearer 认证、失败响应和敏感信息边界。
- [`tests/observability_contract.rs`](../../../tests/observability_contract.rs) 覆盖两个指标 endpoint 的认证、字段、排序、脱敏和无副作用读取。
- [`tests/example_config.rs`](../../../tests/example_config.rs) 覆盖示例配置可解析性。

这些是进程内确定性契约测试，不等同于真实网络部署、外部 SDK、浏览器或生产反向代理验收。

## 相关文档

- [功能需求：网关 API 与客户端兼容](../../functional-requirements/gateway-api-compatibility.md)
- [启动配置与凭证边界](startup-configuration-and-credentials.md)
- [模型信息与能力预检](models-api-and-capability-preflight.md)
- [当前代码架构](../current-architecture.md)
