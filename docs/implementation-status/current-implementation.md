# 当前实现总览

本文只作为 `docs/implementation-status/` 的入口，不再聚合每个功能点的详细实现说明。已完成行为的唯一状态来源是
[功能目录](README.md)中列出的专题文件；每个功能专题分别记录实现事实、代码边界、验证证据和未覆盖范围。

## 已完成的功能点

| 功能点 | 专题文档 |
|---|---|
| HTTP 网关接口与下游认证 | [gateway-http-api-and-auth.md](features/gateway-http-api-and-auth.md) |
| 启动配置、用户与受信凭证边界 | [startup-configuration-and-credentials.md](features/startup-configuration-and-credentials.md) |
| Provider、Model、Target、API、Route 与 Public Model 注册表 | [provider-registry-and-model-catalog.md](features/provider-registry-and-model-catalog.md) |
| Models 接口、Public Model 契约与能力预检 | [models-api-and-capability-preflight.md](features/models-api-and-capability-preflight.md) |
| Chat/Responses Native 转发 | [native-generation-forwarding.md](features/native-generation-forwarding.md) |
| Chat ↔ Responses Protocol Bridge | [protocol-bridge.md](features/protocol-bridge.md) |
| Retry、fallback、credential rotation、cooldown 与取消 | [resilience-retry-fallback-and-cancellation.md](features/resilience-retry-fallback-and-cancellation.md) |
| OpenAI-compatible Embeddings | [embeddings.md](features/embeddings.md) |
| ChatGPT OAuth2 生命周期与 Responses 数据面 | [chatgpt-oauth-startup.md](features/chatgpt-oauth-startup.md) |

## 横向状态文档

- [当前代码架构](current-architecture.md)：描述模块所有权、请求数据流和代码边界。
- [运行时指标与遥测](telemetry-metrics.md)：描述进程内 gateway/provider 快照和可选 OTLP traces 的字段口径与生命周期。
- [上游模型发现与能力探测](capability-probing.md)：描述显式 target probe 的输入、输出和安全边界。
- [协议测试语料与工具](protocol-test-corpus.md)：描述 canonical corpus、Python testkit 和 Rust replay 的验证边界。

## 证据边界

确定性 Rust test、canonical fixture replay、loopback/独立客户端验证、外部 SDK、目标 Agent、真实 Provider、负载和长期运行分别属于
不同证据层。某一层通过不能替代其他层；专题页必须明确写出实际运行的检查和未覆盖的验收层。

当前未完成或不在本轮实现范围的内容包括完整 Native multimodal、ChatGPT 的其他协议/工具/Agent loop、异构协议 Provider、OTLP
metrics/logs、Prometheus、持久化/分布式 observability、动态 health/weight、向量检索以及 GUI/Web 控制面。它们只应在对应功能需求和
当前焦点获准后进入实现。

## 维护规则

新增完成行为时：

1. 先以一个可观察功能点命名专题文件；
2. 只写当前 checkout 已存在的实现事实和已经执行的证据；
3. 把设计目标、未实施想法和外部协议事实分别放回 `functional-requirements/`、`implementation-plans/` 或 `references/`；
4. 更新本目录和相关导航链接，避免在多个专题复制会漂移的模型矩阵或测试数量。

## 相关文档

- [实施现状目录](README.md)
- [当前开发焦点](../implementation-plans/current-focus.md)
- [文档与源码阅读指引](../README.md)
