# OpenBridge 基础目标

## 状态

本文定义当前产品范围。已实现行为和最近验证结果以[当前实现说明](../implementation-status/current-implementation.md)
为准；尚未实现的方向只列为边界，不在这里展开设计。

## 产品目标

OpenBridge 是由单个配置所有者管理、以单个进程部署的 headless Provider 网关。它让登记在私有用户表中的本地 Agent 或
OpenAI-compatible SDK 通过一个稳定的 loopback HTTP 地址调用代码注册的上游服务，同时隐藏上游 credential、endpoint 和内部
Route。

当前核心结果：

- 下游通过 Public Model 调用 `POST /v1/responses`、`POST /v1/chat/completions` 或独立的 `POST /v1/embeddings`；
- Responses 以客户端携带完整历史、`store` 省略或为 `false`、`previous_response_id` 省略或为 `null` 的 无状态调用作为核心兼容面；
- 有状态 Responses 只作为能力受限的 Native pass-through：签发 Upstream Target/Upstream API 必须可唯一 确定，不参与 Bridge
  或跨 Target fallback，OpenBridge 不保存、迁移或恢复上游 response 状态；
- 下游 API Key 匹配启动时加载的不可变用户表，并产生带稳定 user id 的安全请求日志；
- 同协议请求使用 Native Path，保留合法 JSON、HTTP 和 SSE 语义；
- 异协议请求只有在显式 `Bridged` Route 能完整转换 text/function tool 语义时才出站；
- Provider、Model、Upstream Target、Upstream API、Route 与 Public Model 由 Rust 代码显式注册；
- 上游 API key 来自被忽略的私有 upstream credential TOML，下游静态 Bearer token 来自私有用户文件；二者在启动时合并为不可变
  credential 快照；
- 所选 Public Model 先按每 operation 唯一固定契约完成能力预检；通过后 Route 保持配置顺序，不按请求能力筛选或重排；
- 流式请求仅可在首个业务输出前进行有限 retry/fallback；
- 新无状态请求会在单进程内避开短时 cooldown 的 quota/fault scope；
- 已认证请求在 response body 的实际完成、流错误或下游取消边界产生一次脱敏终态观测；高基数诊断事实只进入
  trace，进程内统计只累计低基数终态、attempt 结果和 Provider 明确返回的 usage；
- 管理员可以显式运行 probe，但 probe 不修改注册表或自动扩大能力。

现阶段扩展状态分为：

- 已实现并由确定性 contract/独立 Python loopback 证明：通过独立 Embedding Public Model 调用
  `POST /v1/embeddings`，保持向量身份、编码、维度、顺序与 usage；
- 已批准但尚未进入实施：在 Chat/Responses 同协议 Native Route 中支持已声明的 image、inline/URL file 和 Chat input
  audio，且无资源归属时拒绝 `file_id`。

具体行为和非目标以 [Embeddings 与 Native 多模态扩展需求](embedding-and-native-multimodal.md)为准。这两项不改变
“每次只实施一个可观察行为”的约束；当前 checkout 只提供已在 implementation status 明确记录的能力。

[Model 目录与 Provider 接入配置](model-catalog-configuration.md)已经降级为待定方案，暂不形成产品承诺或实施任务。
在它重新获得明确批准前，当前 Rust 代码注册方式保持不变。

## 静态装配原则

- 配置与代码中的部署决策必须尽可能在启动时完成，生成不可变 registry、用户和 credential snapshot；请求路径不得
  重新解析文件、合并配置或构建新的候选集合；
- Model、Provider 与 Route 候选资格和顺序在启动时固定；不实现按请求动态发现、打分、加权、筛选或重排；
- 不实现用户配额、计费、动态租户策略、配置文件监听、热重载或局部 snapshot 替换；所有配置变化都通过重启生效；
- 固定候选上的有限 retry、fallback、credential rotation 与 cooldown 只用于执行已批准的可用性策略，不得扩大能力、
  改变静态候选资格或演变为动态路由控制面。

## 部署与信任边界

- 默认模型是单配置所有者、单进程和少量受信下游用户；不提供在线用户管理；
- 当前 listener 只允许 loopback；
- 业务请求不能覆盖上游 URL、真实模型、credential、敏感 header 或 Route，也不能选择 header 转换规则；普通 header 只能由
  Provider 的受信代码 hook 按编译期规则处理；
- `RuntimeRegistry` 与 `UserRegistry` 不保存 secret；唯一的 `CredentialStore` 在内存中持有上下游认证所需 Key，Debug
  和日志始终隐藏它；
- 日志、错误、probe report 和测试证据不得暴露 credential 或完整私人请求正文；
- 修改用户、API Key、Provider、Model、Route 或 bootstrap 参数需要重启，不支持热重载。

## 当前接口

| 接口                                                             | 当前用途                                                                                                      |
|------------------------------------------------------------------|---------------------------------------------------------------------------------------------------------------|
| `GET /healthz`                                                   | 返回最小本地存活状态和注册表版本。                                                                            |
| `GET /v1/models`、`GET /v1/models/{model}`                       | 返回代码注册 Public Model 的 OpenAI 标准四字段 list/retrieve。                                                |
| `GET /openbridge/v1/models`、`GET /openbridge/v1/models/{model}` | 按[模型能力契约](model-information-and-capability-contract.md)返回同一目录的模型事实与每 operation 固定能力。 |
| `POST /v1/chat/completions`                                      | 在所选 Public Model 的固定 Chat 契约内按完整 Route 提供 OpenAI-compatible JSON/SSE。                          |
| `POST /v1/responses`                                             | 在所选 Public Model 的固定 Responses 契约内按完整 Route 提供 OpenAI-compatible JSON/SSE。                     |
| `POST /v1/embeddings`                                            | 在独立 Embedding Public Model 的固定契约内按唯一 Native Route 提供有界 JSON 向量结果。                        |

## 扩展接口状态

| 接口                         | 目标用途                                                               | 当前证据入口                                                                                   |
|------------------------------|------------------------------------------------------------------------|------------------------------------------------------------------------------------------------|
| `POST /v1/embeddings`        | 使用独立 Embedding Public Model 提供 OpenAI-compatible JSON 向量结果。 | 当前实现与验证边界见[当前实现说明](../implementation-status/current-implementation.md)。       |
| 现有 Chat/Responses endpoint | 扩展同协议 Native 多模态输入，不扩大 Bridge 或专用媒体 API。           | 需求见[扩展需求](embedding-and-native-multimodal.md)，当前实现边界仍见 implementation status。 |

## 暂不纳入当前产品承诺

- image、structured output、reasoning、Provider 私有扩展或 continuation 的跨协议转换；
- response 状态存储、查询、删除、跨 Provider/Target 迁移和 continuation ledger；
- Responses WebSocket、Realtime、Files、Images、Videos、Conversations 等专用媒体或资源 API；
- OAuth、keyring、加密 secret 文件、远程 secret manager、subscription/OAuth 多账号池、账号级负载均衡和动态 credential
  控制面；未来若重新获准，仍须先满足[上游 OAuth credential lifecycle 条件性安全边界](upstream-oauth-credential-lifecycle.md)
  ，该文档不改变当前非目标；
- 动态权重、持久化/分布式健康、后台探测和多进程协调；
- OpenTelemetry/Prometheus exporter、指标 HTTP API、持久化或分布式聚合；
- hosted tool、MCP Tool Bridge 或由网关执行普通 function tool；
- 多租户、用户管理、配额、计费、审计、GUI 或独立控制面。

本节只限定产品范围，不声明代码缺口。当前实现是否已经覆盖某项核心结果，以
[当前实现说明](../implementation-status/current-implementation.md)为准；新增承诺只有在功能需求先明确、再进入
[当前开发焦点](../implementation-plans/current-focus.md)后才形成实施任务。

## 术语

- **Provider**：代码中实现的一类协议、认证和错误处理行为。
- **Model**：与具体调用 endpoint 分离的模型事实。
- **Credential Pool**：绑定同一 Provider/credential kind 的有序 API-key 集合，可被多个 Target 共享。
- **Upstream Target**：绑定 Provider、Model、endpoint、credential pool、timeout 和故障边界的上游调用边界。
- **Upstream API**：Target 下的一条原生协议供应及其模型名、限制和能力。
- **Route**：固定下游协议、Upstream Target、Upstream API 和执行模式的路径。
- **Public Model**：下游使用的稳定模型身份、每协议固定能力契约及私有有序 Routes。
- **Native Path**：上下游协议一致时的最小改写转发路径。
