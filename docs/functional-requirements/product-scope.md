# OpenBridge 基础目标

## 状态

本文定义当前产品范围。已实现行为和最近验证结果以[当前实现说明](../implementation-status/current-implementation.md)
为准；尚未实现的方向只列为边界，不在这里展开设计。

## 产品目标

OpenBridge 是由单个配置所有者管理、以单个进程部署的 headless Provider 网关。它让登记在私有用户表中的本地 Agent 或
OpenAI-compatible SDK 通过一个稳定的 loopback HTTP 地址调用代码注册的上游服务，同时隐藏上游
credential、endpoint 和内部 Route。

当前核心结果：

- 下游通过 Public Model 调用 `POST /v1/responses` 或 `POST /v1/chat/completions`；
- 下游 API Key 匹配启动时加载的不可变用户表，并产生带稳定 user id 的安全请求日志；
- 同协议请求使用 Native Path，保留合法 JSON、HTTP 和 SSE 语义；
- 异协议请求只有在显式 `Bridged` Route 能完整转换 text/function tool 语义时才出站；
- Provider、Model、Upstream Target、Upstream API、Route 与 Public Model 由 Rust 代码显式注册；
- 上游 API key 来自受限环境变量或被忽略的 `.env`，下游静态 Bearer token 来自私有用户文件；二者在启动时合并为不可变 credential 快照；
- Route 按完整协议、能力、模型限制和状态亲和要求确定性筛选；
- 流式请求仅可在首个业务输出前进行有限 retry/fallback；
- 新无状态请求会在单进程内避开短时 cooldown 的 quota/fault scope；
- 管理员可以显式运行 probe，但 probe 不修改注册表或自动扩大能力。

## 部署与信任边界

- 默认模型是单配置所有者、单进程和少量受信下游用户；不提供在线用户管理；
- 当前 listener 只允许 loopback；
- 业务请求不能覆盖上游 URL、真实模型、credential、认证/非 allowlist header 或 Route；普通 header 只能由 Provider 的受信代码 hook 显式选择；
- `RuntimeRegistry` 与 `UserRegistry` 不保存 secret；唯一的 `CredentialStore` 在内存中持有上下游认证所需 Key，Debug 和日志始终隐藏它；
- 日志、错误、probe report 和测试证据不得暴露 credential 或完整私人请求正文；
- 修改用户、API Key、Provider、Model、Route 或 bootstrap 参数需要重启，不支持热重载。

## 当前接口

| 接口 | 当前用途 |
|---|---|
| `GET /healthz` | 返回最小本地存活状态和注册表版本。 |
| `GET /v1/models` | 返回代码注册的 Public Model。 |
| `POST /v1/chat/completions` | OpenAI-compatible Chat JSON/SSE Native Path。 |
| `POST /v1/responses` | OpenAI-compatible Responses JSON/SSE Native Path。 |

## 当前未实现

- image、structured output、reasoning、Provider 私有扩展或 continuation 的跨协议转换；
- Responses WebSocket、Realtime、Files、Conversations 等资源 API；
- OAuth、keyring、私有 secret 文件和多 credential pool；
- 动态权重、持久化/分布式健康、后台探测和多进程协调；
- 调用统计、usage 聚合、TTFT/TTFB 和指标导出；
- hosted tool、MCP Tool Bridge 或由网关执行普通 function tool；
- 多租户、用户管理、配额、计费、审计、GUI 或独立控制面。

这些能力只有在进入[当前开发焦点](../implementation-plans/current-focus.md)后才形成实施计划。

## 术语

- **Provider**：代码中实现的一类协议、认证和错误处理行为。
- **Model**：与具体调用 endpoint 分离的模型事实。
- **Upstream Target**：绑定 Provider、Model、endpoint、credential 和 timeout 的上游调用边界。
- **Upstream API**：Target 下的一条原生协议供应及其模型名、限制和能力。
- **Route**：固定下游协议、Upstream Target、Upstream API 和执行模式的路径。
- **Public Model**：下游使用的稳定模型名及其有序 Routes。
- **Native Path**：上下游协议一致时的最小改写转发路径。
