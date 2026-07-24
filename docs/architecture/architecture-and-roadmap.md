# OpenBridge 单用户 Provider 聚合代理：目标架构与收敛路线

## 状态

**Working hypothesis。** 部分 HTTP/SSE、route snapshot、capability gate 和 fallback 假设已由原型验证；Provider 抽象、目标客户端契约和 bridge 状态模型仍需外部反例与真实 fixture 收敛。

## 1. 架构结论

OpenBridge 默认是一个单进程、单用户、单配置所有者的服务。它不拆分独立控制面和数据面，也不建立 tenant/principal/配额/合规审计系统。

逻辑架构：

```text
Codex / Hermes / OpenAI-compatible client
                    │
                    ▼
┌────────────────────────────────────────────┐
│ HTTP ingress (JSON/SSE)                    │
│ /v1/responses  /v1/chat/completions        │
│ body limit / request id / optional token   │
└──────────────────────┬─────────────────────┘
                       ▼
┌────────────────────────────────────────────┐
│ Request classifier                         │
│ protocol / alias / requested capabilities  │
└──────────────────────┬─────────────────────┘
                       ▼
┌────────────────────────────────────────────┐
│ Route planner                              │
│ alias → ordered candidates                 │
│ capability filter / state affinity         │
│ native-or-bridge decision                  │
└───────────────┬────────────────────────────┘
                │ immutable RoutePlan
         ┌──────┴────────┐
         ▼               ▼
┌────────────────┐  ┌────────────────────────┐
│ Native Path    │  │ Protocol Bridge        │
│ minimal rewrite│  │ wire → Bridge IR → wire│
│ preserve wire  │  │ supported subset only  │
└───────┬────────┘  └───────────┬────────────┘
        └────────────┬──────────┘
                     ▼
┌────────────────────────────────────────────┐
│ Provider adapter + shared HTTP/SSE transport│
└──────────────────────┬─────────────────────┘
                       ▼
                 Upstream Provider
```

同一个进程内可以存在配置、usage sink 和 future hosted-tool 模块，但它们不形成企业级控制平面。首版下游 transport 是 HTTP JSON/SSE；Responses WebSocket 保留为显式扩展点，而不是默认兼容承诺。

## 2. 核心数据模型

### 2.1 Provider Family

代码中实现的一类协议和认证行为：

```text
ProviderFamily
  id
  native_protocols
  request adapter(s)
  response/SSE adapter(s)
  auth/header adapter(s)
  error classifier
  capability upper bound
```

首批建议：

```text
openai
openai-compatible
anthropic
```

一个 Provider Family 可以服务多个运行时 deployment。对轻微 header/path 差异优先复用公共 adapter，而不是复制完整 transport。

### 2.2 Deployment

受信配置中的一个上游目标：

```text
Deployment
  id
  provider_family
  base_url
  credential_ref
  upstream_model
  native_protocols
  native_transports
  capabilities
  timeout
  enabled
```

服务所有者可以配置 base URL 和 adapter 明确允许的少量非认证 header。业务请求不能覆盖这些值。

### 2.3 Public Model Alias

下游客户端使用的稳定名称：

```text
PublicModelAlias
  name
  candidates: ordered deployment ids
```

第一版 candidate 顺序就是确定性优先级。alias 不应隐式承诺所有 candidate 完全等价；route planner 仍需按协议、请求 feature 和 state affinity 过滤。

### 2.4 RoutePlan / RouteSnapshot

单次请求固定的不可变结果：

```text
RoutePlan
  request_id
  public_alias
  selected_deployment
  remaining_eligible_candidates
  downstream_protocol
  downstream_transport
  upstream_protocol
  upstream_transport
  mode: native | bridge
  capability_decision
  credential_binding_id
  continuation_binding
  fallback_boundary
  config_version
```

配置 reload 只影响后续请求；进行中的 stream 继续持有原 snapshot。

## 3. Native Path 与 Protocol Bridge

### 3.1 Native Path

适用于：

```text
Responses → Responses
Chat Completions → Chat Completions
```

行为：

- 解析路由所需的 `model`、`stream` 和 feature indicators；
- 将 public alias 改写为 `upstream_model`；
- 构造受信 URL、认证和必要 Provider header；
- 尽量保留未知但合法的 request/response 字段；
- 对 SSE 做 framing、terminal/error 识别和取消传播；
- 不先完整转换为通用 IR 再重新渲染。

这样可降低新字段丢失、目标客户端版本漂移和不必要分配。

### 3.2 Protocol Bridge

适用于：

```text
Responses → Chat Completions
Chat Completions → Responses
Chat/Responses → Anthropic Messages
```

只转换明确支持的语义。首个 bridge slice：

- text message/item；
- function tool schema；
- function call；
- tool result；
- usage；
- terminal outcome 的最小映射。

以下默认不是首个 slice 的等价能力：

- Provider-hosted tool；
- resource/background API；
- Provider-bound continuation；
- opaque reasoning/encrypted content；
- 未知 source-specific item；
- 无法保持 identity 的并行工具调用；
- 未经验证的 multimodal item。

详见 [Chat/Responses bridge 设计](../design/chat-responses-conversion.md)。

## 4. Provider adapter 与共享 transport

建议模块边界：

```text
src/
  core/           # IDs, RoutePlan, errors, capability
  config/         # trusted owner configuration and snapshots
  ingress/        # HTTP endpoints, request id, body limits, static token
  routing/        # alias resolution, eligibility, selection, fallback policy
  protocol/       # wire models and Bridge IR
  bridge/         # protocol-pair converters and stream assemblers
  provider/       # adapter traits and ProviderFamily catalog
  providers/      # concrete provider families
  transport/      # shared HTTP client, SSE framing, cancellation
  usage/          # optional bounded UsageRecord sink
```

依赖方向应避免 ingress/router 识别具体 Provider header 或 token 形状。Provider adapter 也不应自行决定 public alias 和 candidate 顺序。

共享 transport 负责：

- 连接池；
- 禁止不受控 redirect；
- request/response body limits；
- SSE framing；
- deadline 与 cancellation；
- 首输出前 retry boundary；
- 安全响应 header 保留。

Provider adapter 负责：

- path/query/body 差异；
- auth 与固定/受限 header；
- Provider wire response/SSE event；
- terminal 和错误分类；
- capability upper bound。

## 5. Capability 模型

能力分成两层：

1. deployment/model 记录上游原生协议、transport 和 feature 能力：`Supported | Unsupported | Unknown`，并受 Provider Family 的 capability upper bound 约束；
2. route planner 针对具体下游协议和请求 feature 计算有效结果：

```text
Native | Bridged | Unsupported | Unknown
```

`Bridged` 不是服务所有者可以凭配置声明的 Provider 事实；它必须由已实现的协议对 converter、目标协议和 feature preservation rule 推导。

route planner 的判断顺序：

```text
alias candidates
→ endpoint/protocol/transport eligibility
→ requested-feature capability
→ continuation/state affinity
→ enabled/cooldown state
→ ordered selection
```

`Unknown` fail closed；只有明确 fixture 或官方契约支持后才升为 `Native`/`Bridged`。

能力不是 Provider 名称的静态布尔值，至少受 deployment、model、endpoint/API version 和 feature combination 影响。第一版可以用显式配置和测试固定结果，不必建立动态能力发现服务。

## 6. 路由、attempt 与 fallback

将“路由”拆成四个步骤：

1. **Eligibility**：协议、capability、credential 和 state 是否允许；
2. **Selection**：从合格 candidate 中按配置顺序选第一个；
3. **Attempt policy**：哪些首输出前失败允许重试/下一个 candidate；
4. **Continuation policy**：有状态请求是否必须回到 issuing deployment。

核心第一版允许 fallback 的典型情况：

- connect/DNS/TLS failure；
- 明确 429 或配置允许的 5xx；
- response body 尚未交给下游时的 timeout；
- 上游未产生任何可观察业务输出。

禁止透明 fallback：

- 已输出任何业务 JSON/SSE；
- 请求携带 `previous_response_id` 或 Provider resource ID；
- tool result 正在回复特定 issuing call；
- 上游可能已执行有副作用的 Provider-hosted action；
- 无法判断上游是否已接受请求且重复可能产生副作用。

可在核心后加入被动 cooldown，不需要主动健康检查集群。

## 7. 状态所有权

| 状态 | 所有者 | 生命周期 | 可否跨 deployment |
|---|---|---|---|
| RoutePlan | OpenBridge request | 单请求/stream | 否 |
| SSE assembly | protocol adapter/bridge | 单 stream | 否 |
| Tool identity map | Protocol Bridge | 单请求或明确 continuation ledger | 默认否 |
| `previous_response_id` | issuing Provider/deployment | Provider 定义 | 否 |
| Credential material | service owner + credential source | 配置/refresh 生命周期 | 仅绑定对应 deployment |
| UsageRecord | optional local sink | 请求结束后 | 可聚合，但不参与路由 |
| Hosted tool session | future facade | 单 tool call/Provider contract | 默认否 |

任何跨请求 ledger 都必须有 issuer、deployment、expiry 和歧义拒绝规则。第一版可以对无法安全恢复的 continuation 直接拒绝，而不是为了兼容而建立隐式全局 cache。

## 8. 配置与网络边界

配置分两类：

### 启动级配置

- listen address；
- request/SSE limits；
- upstream origin policy；
- connection pool；
- optional downstream static token reference；
- route config path。

### 可 reload 路由配置

- deployments；
- credential references；
- model aliases；
- capability declarations/overrides；
- timeout、enable state 和 candidate 顺序。

reload 必须构建并验证完整新 snapshot 后原子替换。受信配置可定义兼容 endpoint，但不能注入任意代码、模板转换或动态脚本。

详细配置设计见[本地配置、路由与使用量](local-configuration-routing-and-usage.md)。

## 9. 目标客户端

- Codex：P0 native Responses over HTTP/SSE；使用独立 custom Provider id，并显式配置 `supports_websockets = false`；
- Hermes：P0 native Chat，P1 native Responses；
- bridge 优先解决 Codex Responses → Chat-only Provider，再验证 Hermes Chat → Responses-only Provider。

测试以完整 Agent tool loop 为核心，不以“SDK 能解析一次响应”替代。详见[目标客户端契约](../design/target-client-contracts.md)。

## 10. 调研与实施收敛路线

### Gate C0：范围与客户端契约

产物：

- 单用户核心/非目标；
- 固定 Codex 与 Hermes 版本；Codex 同时固定 custom Provider 配置和 HTTP/SSE transport profile；
- native/bridge 兼容矩阵；
- 原始请求与 SSE corpus 结构。

### Gate C1：双 Native Path

产物：

```text
Codex Responses HTTP/SSE → OpenAI Responses-compatible upstream
Hermes Chat → OpenAI-compatible Chat upstream
```

退出条件：两个目标 Agent 各完成一个真实多轮 function tool loop，并覆盖 cancel/error/EOF；Codex 诊断确认 custom Provider 未启用 WebSocket。

### Gate C2：Provider 聚合核心

产物：

- Provider Family + Deployment + Alias + RoutePlan；
- 至少两个 Provider Family；
- capability filtering；
- ordered candidate 与首输出前 fallback；
- state affinity。

### Gate C3：Responses → Chat Bridge

让 Codex Responses HTTP/SSE 通过 Chat-only Provider 完成文本和普通 function tool loop。每个不等价能力有明确的 reject/approximation classification。

### Gate C4：Chat → Responses Bridge

让 Hermes Chat transport 通过 Responses-only Provider 完成文本和普通 function tool loop，并验证反向 identity、status 和 stream renderer。

### Gate C5：异构 Provider 验证

引入 Anthropic Messages 或等价 archetype，验证：

- 内容块；
- tool use/tool result identity；
- stop reason；
- streaming event；
- Provider-specific error。

如果 Provider Family、Bridge IR 或 state model 必须调整，应在此 gate 完成，而不是继续堆叠 Provider。

### Gate C6：核心接受

固定目标客户端 corpus、至少三个 Provider archetype、安全与资源基线、Provider onboarding、操作文档和发布/回滚流程，形成可发布的核心版本。

### Enhancement E1+

核心稳定后依次考虑：

1. UsageRecord JSONL/SQLite；
2. 被动 cooldown；
3. Hosted Tool Facade；
4. Tool Bridge/MCP；
5. optional OAuth；
6. UI。

Hosted tool 不依赖 Protocol Bridge 完成；其前置是 native hosted-tool Provider route、能力声明、取消/超时、结果/citation 契约和最小使用量记录。

## 11. 质量门

| 层次 | 最小验证 |
|---|---|
| Unit | config、alias、capability、SSE parser、error classification |
| Contract fixture | 每个 Provider Family 的 JSON/SSE/tool/error corpus |
| Client compatibility | 固定 Codex/Hermes 版本的完整 tool loop |
| Bridge property | identity、ordering、terminal、round-trip preservation class |
| Security boundary | client 不能覆盖 URL/header/credential；secret scan；non-loopback token |
| Resource behavior | cancellation、slow consumer、有界 buffer、首输出前 fallback |

## 12. 已拒绝或延期的方向

- **所有请求统一进入 Bridge IR**：拒绝作为默认路径；Native Path 应保留 wire 兼容性。
- **完全运行时 JSON Provider 行为**：当前拒绝；协议/auth/转换仍由代码实现，运行时只配置 deployment 数据。
- **每个兼容 endpoint 都编译成独立 Provider enum**：拒绝；同一 Provider Family 应允许多个受信 deployment。
- **企业级 key/principal/配额/审计作为核心**：拒绝；不符合单用户目标。
- **真实 Codex subscription OAuth 作为主线前置**：延期/Blocked；API key 路径独立推进。
- **Responses WebSocket 作为首版核心 transport**：延期；先验证 Codex custom Provider 的 HTTP/SSE profile，触发条件见目标客户端契约。
- **Hosted Tool/MCP 进入核心关键路径**：延期；在聚合和 bridge 核心稳定后实施。

## 13. 关联文档

- [核心需求](../requirements/proxy-requirements.md)
- [目标客户端契约](../design/target-client-contracts.md)
- [Rust Provider adapter 与数据流](rust-provider-adapter-dataflow.md)
- [本地配置、路由与使用量](local-configuration-routing-and-usage.md)
- [Chat/Responses bridge](../design/chat-responses-conversion.md)
- [开发与调研收敛计划](../plans/development-plan.md)
- [当前实现说明](../implementation/current-implementation.md)
