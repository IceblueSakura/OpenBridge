# 配置与凭证域

本域定义 Bootstrap、代码注册表、credential 与受信 egress 边界；不记录实现完成度或验证结果。

| 功能模块 | 文档 |
|---|---|
| 所有权与代码注册表 | [ownership-and-registry.md](ownership-and-registry.md) |
| API-key pool 与 OAuth credential | [credentials.md](credentials.md) |
| Endpoint 与 egress | [endpoint-and-egress.md](endpoint-and-egress.md) |
| 启动与运行生命周期 | [lifecycle.md](lifecycle.md) |
| 功能验收要求 | [acceptance.md](acceptance.md) |
| ChatGPT subscription OAuth | [upstream-oauth-credential-lifecycle.md](upstream-oauth-credential-lifecycle.md) |

Provider contract、Model、Target、Upstream API、Route、Public Model、endpoint、能力与 wire mapping 由受信 Rust
代码显式注册；运行时配置不提供 Provider DSL，也不支持 route hot reload。

Registry、用户、API-key store 与 OAuth manager wiring/locator 在启动时校验并冻结。OAuth manager 内部 token
snapshot/generation 可以按其专有 lifecycle guarded refresh/rotation；这不是 registry、Route、账户 binding 或配置
热重载。

业务请求不能选择 endpoint、credential、header policy 或 routing topology。实现与验证事实见
[实施现状](../../implementation-status/README.md)。

## 关联文档

- [Public Model 与模型能力契约](../model-capability/README.md)
- [路由与 Provider 韧性](../routing-resilience/README.md)
- [运行期观测](../observability/README.md)
- [实施现状](../../implementation-status/README.md)
