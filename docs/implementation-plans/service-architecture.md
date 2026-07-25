# OpenBridge 单用户 Provider 聚合代理：服务架构

## 状态

**Working hypothesis。** 部分 HTTP/SSE、route snapshot、capability gate 和 fallback 假设已由原型验证；Provider 抽象、目标客户端契约和 bridge 状态模型仍需外部反例与真实 fixture 收敛。

## 1. 架构结论

OpenBridge 默认是一个单进程、单用户、单配置所有者的服务。它不拆分独立控制面和数据面，也不建立 tenant/principal/下游配额/合规审计系统。

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
```

Chat/Responses → Anthropic Messages 是后续再评估的独立兼容方向，与 Provider-hosted tool facade 同级，不属于初始 bridge 范围。

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

详见 [Chat/Responses bridge 设计](protocol-bridge.md)。

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

`enabled/cooldown state` 是运行时 availability overlay，不写回配置 snapshot。RoutePlan 固定 candidate identity、顺序、credential binding 和 fallback 边界；attempt manager 在实际调用前读取最新 cooldown，以避免把动态 Provider 状态伪装成静态 capability。

能力不是 Provider 名称的静态布尔值，至少受 deployment、model、endpoint/API version 和 feature combination 影响。第一版可以用显式配置和测试固定结果，不必建立动态能力发现服务。

## 6. 路由、attempt 与 fallback

将“路由”拆成四个步骤：

1. **Eligibility**：协议、capability、credential 和 state 是否允许；
2. **Selection**：从合格 candidate 中按配置顺序选第一个；
3. **Attempt policy**：哪些首输出前失败允许在次数、等待和总耗时预算内重试/下一个 candidate；
4. **Continuation policy**：有状态请求是否必须回到 issuing deployment。

核心第一版允许 fallback 的典型情况：

- connect/DNS/TLS failure；
- 明确 429 或 adapter 认可的临时 5xx；
- response body 尚未交给下游时的 timeout；
- 上游未产生任何可观察业务输出。

禁止透明 fallback：

- 已输出任何业务 JSON/SSE；
- 请求携带 `previous_response_id` 或 Provider resource ID；
- tool result 正在回复特定 issuing call；
- 上游可能已执行有副作用的 Provider-hosted action；
- 无法判断上游是否已接受请求且重复可能产生副作用。

Provider 聚合相关实现应提供最小被动 cooldown：429 与明确临时不可用会使 deployment 在有界时间内退出无状态 selection，优先使用 `Retry-After`/rate-limit reset，没有有效 header 时使用有界 backoff + jitter。所有 candidate cooling down 时返回明确 429 和可确定的最早恢复时间。

主动探测、跨进程 cooldown、一致性限流和自适应权重仍属于核心后的增强。详细错误分类、retry budget、state affinity 和下游错误传播见[Provider 韧性需求](../functional-requirements/provider-resilience.md)。

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

详细配置设计见[配置与路由](configuration-and-routing.md)。

## 9. 目标客户端

- Codex：初期 native Responses over HTTP/SSE；使用独立 custom Provider id，并显式配置 `supports_websockets = false`；
- Hermes：仅在需要兼容声明时分别验证 native Chat 或 Responses；
- bridge 优先解决 Codex Responses → Chat-only Provider，再验证 Hermes Chat → Responses-only Provider。

日常开发验证优先使用当时可用 OpenAI SDK 的完整 wire/tool-loop 测试，以及当时可用 Codex CLI 的 Responses custom Provider E2E；每次记录实际版本和环境，不建立长期版本 pin。SDK 不是“只解析一次响应”的替代品。Hermes 作为显式兼容声明的补充 E2E，而不是每轮开发的首选入口。详见[客户端兼容](client-compatibility.md)。

## 10. 发展方向（非路线图）

以下是可以由实际用户需求触发的技术方向，不构成固定开发顺序、阶段或完成承诺。每次只选择一个可观察行为进入 TDD 当前焦点。

| 方向 | 可选择的首个行为示例 | 主要验证方式 |
|---|---|---|
| 原生转发 | 一个 Chat 或 Responses 的 JSON/SSE 边界、错误或取消语义 | Rust fixture + OpenAI SDK；Responses 路径再用 Codex CLI |
| Provider 聚合与韧性 | alias 候选筛选、session affinity、429 cooldown、有限 retry 或安全错误转发 | 确定性 fixture；按 Provider 需要补真实上游 |
| Protocol Bridge | 一个明确可表达的文本或普通 function-tool 语义 | 双向 fixture；受影响客户端的 SDK/CLI 观察 |
| Hosted Tool Facade | 一个 native hosted tool 的输入、终态或 citation 规范化 | Provider fixture 与目标 MCP client 观察 |
| Anthropic Messages | 一个 content block、tool use/result 或 stream event 的可表达性 | 协议 fixture；必要时真实 Messages Provider |
| 观测与辅助能力 | usage 记录、健康状态、OAuth 或 UI 的一个独立用户结果 | 与该结果相称的单元和集成测试 |

Hosted tool facade 与 Anthropic Messages 没有预设先后。它们都不应在缺少明确用户结果和失败测试时进入实现。

## 11. 按行为选择验证

| 层次 | 适用时机 |
|---|---|
| Unit | config、alias、capability、SSE parser、error classification 等局部行为。 |
| Contract fixture | JSON/SSE/tool/error 的确定性回归。 |
| OpenAI SDK | Chat/Responses 的客户端可见 HTTP/SSE 行为。 |
| Codex CLI | custom Provider 的 Responses transport、错误与 tool loop。 |
| Bridge property | identity、ordering、terminal 与保留等级。 |
| Security/resource | URL/header/credential 边界、cancel、slow consumer、有界 buffer、首输出前 fallback。 |

运行时记录实际 SDK/CLI 版本和环境，但不作长期版本锁定。Hermes 只在对应行为需要支持它时追加验证。

## 12. 关联文档

- [产品范围](../functional-requirements/product-scope.md)
- [客户端兼容](client-compatibility.md)
- [Provider 适配与数据流](provider-adapters-and-dataflow.md)
- [配置与路由](configuration-and-routing.md)
- [Chat/Responses bridge](protocol-bridge.md)
- [交付与证据要求](../functional-requirements/delivery-and-evidence.md)
- [当前实现说明](../implementation-status/current-implementation.md)
