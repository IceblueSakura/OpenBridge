# 配置与凭证域

Bootstrap、代码注册表、凭证与受信边界需求按功能模块拆分如下。子文档只定义目标行为、失败语义与安全边界；不记录
"代码已经做到什么"或某次测试结果。

| 功能模块                              | 文档                                                    |
|---------------------------------------|---------------------------------------------------------|
| 所有权划分与代码注册表                | [ownership-and-registry.md](ownership-and-registry.md)  |
| 凭证（API-key pool、ChatGPT 隔离、OAuth2 auth 文件） | [credentials.md](credentials.md)          |
| Endpoint 与出站边界                   | [endpoint-and-egress.md](endpoint-and-egress.md)        |
| 生命周期                              | [lifecycle.md](lifecycle.md)                            |
| 功能验收要求                          | [acceptance.md](acceptance.md)                          |
| ChatGPT subscription OAuth 生命周期   | [upstream-oauth-credential-lifecycle.md](upstream-oauth-credential-lifecycle.md) |

## 状态

**当前约束。** OpenBridge 是单配置所有者管理的 headless 网关。Provider contract、Model、 Upstream Target、Upstream
API、Route、Public Model、endpoint、能力和字段转换由 Rust 代码显式注册；运行时配置不提供 Provider DSL，也不支持 route 热重载。

[Model 目录与 Provider 接入配置](../pending/model-catalog-configuration.md)目前是待定方案，不属于本域当前约束，也不进入
实施。除非再次明确批准，启动过程、注册表所有权和验收要求继续以代码注册方式为准。

## 域目标（用户结果）

本域保证配置、凭证与受信运行边界的可验证性：所有 Provider/Model/Route 事实由代码注册表在启动时一次性校验并冻结，
secret 只进入启动时不可变存储并按用途受限借用，业务请求不能控制 endpoint、credential 或路由拓扑。

## 关联文档

- [Public Model 与模型能力契约](../model-capability/README.md)
- [待定 Model 目录与 Provider 接入配置](../pending/model-catalog-configuration.md)
- [ChatGPT subscription OAuth credential lifecycle](upstream-oauth-credential-lifecycle.md)
- [当前代码架构](../../implementation-status/current-architecture.md)
- [当前实现总览](../../implementation-status/current-implementation.md)
- [Models 与基础 API 探测](../../implementation-status/capability-probing.md)
