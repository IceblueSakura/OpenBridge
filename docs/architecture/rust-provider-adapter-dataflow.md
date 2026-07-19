# Rust provider adapter 与数据流架构

## 状态

**已确认，实施中。** 当前已有 Rust crate、闭合 provider catalog、typed route snapshot、provider trait/credential lease contracts 与 shared SSE framing；完整 provider transport/pipeline 仍按后续阶段实施。

## 1. 决策

系统使用 Rust 开发，并采用：

> **编译期 provider catalog + 运行时小型路由配置 + 有类型异步数据流 pipeline**

不采用 LiteLLM 风格的 JSON Registry 来定义 provider 行为。每个 provider 的请求结构、特定 header、认证头、响应/SSE 解析、错误映射和 capability 由 Rust adapter trait 的实现表达；运行时配置只选择已编译 provider、部署地址、公开 alias、timeout、启用状态和 secret reference。

这不意味着完全不使用 JSON：OpenAI-compatible HTTP request/response 和上游 wire payload 仍然是 JSON。拒绝的是将 provider 行为、header 规则、协议转换逻辑和安全策略写成运行时解释的 JSON 数据。

## 2. 目标和非目标

### 目标

- 为新增 provider 提供有边界、可编译、可 fixture-test 的 adapter 扩展点。
- 让 provider 特定 header 规则在代码审查、类型检查和测试覆盖中可见，而不是隐藏在通用配置中。
- 让 request/stream/response 沿有类型的异步数据流单向传播，保留 cancellation 与 backpressure。
- 避免在 token/event 热路径进行动态 JSON registry 查找、任意 header 注入、反射式参数过滤或无界 buffer。
- 多 provider 仍可由公开 alias 路由；当前每 provider 只绑定一个 active credential。

### 非目标

- 不实现运行时加载 Rust plugin、动态链接 provider 二进制或任意第三方脚本。
- 不允许配置添加未审计的 provider header、任意 upstream URL、认证策略或 body transform。
- 不为未来同 provider 多 credential 提前建立 credential selector / pool。
- 不把每一个轻微 provider 差异拆成独立 transport；优先共享 HTTP、SSE、retry、cancellation 和 audit pipeline。

## 3. Rust 模块边界

初始建议保持**单个 Cargo crate 的模块化结构**，避免项目早期因 workspace/crate 边界造成额外迁移。provider 数量、feature flags 或依赖隔离确实需要时，再拆为 workspace crates。

```text
src/
  core/             # IDs, request context, route snapshot, errors, capability model
  protocol/         # Chat/Responses wire models, Canonical IR, SSE event types
  pipeline/         # stage contracts, stream orchestration, cancellation, backpressure
  transport/        # shared HTTP client, SSE framing, retry before first downstream event
  provider/         # adapter traits, ProviderKind, credential lease abstraction
  providers/        # compiled provider implementations
    openai.rs
    codex.rs
    ...
  control/          # route config, vault interface, credential lifecycle, admin APIs
  ingress/          # OpenAI-compatible HTTP handlers
  observability/    # audit observation/outbox, metrics, redaction
```

依赖方向必须是单向的：

```text
protocol/core → pipeline → provider traits → provider implementations
                                   ↑
control / ingress / observability ─┘
```

`providers/*` 可以依赖 `provider`、`protocol`、`transport`，但 `core`、`protocol`、`pipeline` 不能依赖具体 provider。HTTP handler 不能识别 provider-specific header 或 OAuth token 形状。

## 4. Provider adapter：按能力分解 trait

不要定义包含所有 provider 可能性的巨型 `Provider` trait，也不要用大量 `Option`、feature flag 或 provider name 分支填充公共 pipeline。将 provider 差异拆成小 trait；一个 provider 只实现它真正需要的部分。

| Trait / role | 输入 → 输出 | 职责 |
|---|---|---|
| `ProviderDescriptor` | static metadata | `ProviderKind`、支持 endpoint、capability profile、配置 schema version |
| `RequestAdapter` | normalized request → upstream request | 将 Chat/Responses/Canonical IR 映射成上游 path、query、JSON body |
| `AuthAdapter` | credential lease + request parts → headers | 仅在发送前构造 bearer、account/workspace 或 provider 特定 header |
| `HeaderAdapter` | route/request metadata → safe headers | provider 必需的非认证 header；规则编译在代码中 |
| `ResponseAdapter` | HTTP response/SSE frame → normalized event | 解析上游 JSON、SSE event、usage、terminal state |
| `ErrorAdapter` | upstream failure → proxy error class | 映射 retryability、OpenAI-style error envelope、safe message |
| `CapabilityAdapter` | request + deployment profile → decision | 在发送前拒绝无能力 feature，避免静默 drop 参数 |

`RequestAdapter`、`ResponseAdapter` 和 `ErrorAdapter` 是 provider extension 的主入口。若某 provider 与标准 OpenAI-compatible upstream 完全一致，应复用公共 adapter，再只实现必要的 `HeaderAdapter` 或 `AuthAdapter`；不要复制整个 transport 链。

### Provider catalog

运行时多 provider 需要异构 dispatch，但不要求在热路径使用 `HashMap<String, Box<dyn Provider>>`。初期使用闭合 enum：

```text
ProviderKind = OpenAi | Codex | ...
ProviderAdapter = OpenAi(OpenAiAdapter) | Codex(CodexAdapter) | ...
```

route config 解析时将 provider id 转为 `ProviderKind`；未知值使启动/配置 reload 失败。请求开始时把对应 `ProviderAdapter` 放进不可变 `RouteSnapshot`，通过 enum `match` 调用 trait 实现。这样 provider 行为是编译期可见的；一次 enum dispatch 的成本远低于网络、TLS、JSON 和模型 TTFT，仍应以 benchmark 验证而非假设。

在 provider 数量显著增加、feature-gated build 或单独依赖确有需要前，不引入运行时 plugin registry。若以后必须换成 trait object，边界仅可位于 route snapshot 构建处，不能泄漏到每个 SSE token/event 的处理路径。

## 5. 数据流 pipeline

核心处理必须表达为显式单向数据流，而不是在 HTTP handler、router、provider client、logger 间共享可变状态。

```text
InboundHttpRequest
  → ValidatedRequest
  → AuthorizedRequest
  → RoutedRequest(RouteSnapshot)
  → ProviderRequest
  → UpstreamHttpResponse / UpstreamByteStream
  → NormalizedResponse / NormalizedEventStream
  → ClientResponse / ClientSseStream

                     └→ AuditObservation → bounded outbox
```

每个 stage：

- 接收一个已定义输入 envelope，返回下一个 envelope 或明确的 `ProxyError`；
- 不依赖全局 provider name 分支；provider 差异只经 adapter trait 注入；
- 将 cancellation token / request deadline 向下游传播；
- 对 stream 采用 pull-based transform 或有界 channel，不能为方便 fan-out 创建无界 channel；
- 只在需要跨异步边界共享时使用 `Arc`；byte payload 以 `Bytes`/borrowed view 传递，避免每个 token 拷贝；
- 在 route、credential、capability、audit 中保存 immutable snapshot，避免 config reload 改变进行中的 stream。

SSE framing 与 JSON event decoding 属于 transport/response stage；业务层只能消费完整 event，不能读取任意网络 chunk。审计是旁路 data flow：默认只接收 metadata 的 `AuditObservation`，内容 capture 需单独 scope/policy，且 outbox 必须有界。

## 6. 运行时配置的允许范围

当前 bootstrap policy（listen、upstream origin allowlist、request/SSE size limits）只在启动时建立；显式 reload 仅允许替换 route config。bootstrap policy 变化必须拒绝 reload，直到 router 与 runtime policy 能作为同一原子 snapshot 一起替换。

允许以 TOML/YAML/环境变量等静态配置承载**数据**：

```text
provider kind              # 必须是编译期 ProviderKind
base URL                   # 必须通过 allowlist
public model alias / deployment mapping
timeout / enable state / priority / weight
credential secret reference
capability override        # 仅可收窄，不能声明代码不支持的能力
```

禁止由配置承载**行为**：

```text
arbitrary header map
arbitrary auth scheme
arbitrary request/response transform
arbitrary retry classification
provider-specific body template
client-supplied base URL / header forwarding rule
```

特定 header（例如 provider 的 account、version、beta 或 client metadata header）必须在对应 `HeaderAdapter`/`AuthAdapter` 中声明，并有正反 fixture：正确 header 被发送，secret/header 不泄露，错误 route 不会收到该 header。

## 7. 各阶段的实现影响

| 阶段 | Rust / trait / dataflow 交付 |
|---|---|
| Phase 0 | `core`/`protocol` 边界；shared SSE framing；闭合 `ProviderKind`/`ProviderAdapter` catalog；typed route snapshot；provider trait/descriptor contracts；opaque `CredentialLease` 值对象与 fixture tests |
| Phase 1 | 第一个标准 OpenAI-compatible adapter；native Chat/Responses request/response pipeline；retry/cancel/backpressure 测试 |
| Phase 2 | Codex `AuthAdapter`/`HeaderAdapter`；credential store/vault、login、refresh state machine；token 不流入 audit payload |
| Phase 3 | 扩展多 provider catalog；alias/routing policy；多 provider conformance suite |
| Phase 4 | `AuthorizedRequest` stage、opaque key verifier、principal scope/limit stage；不影响 provider trait |
| Phase 5 | `AuditObservation` side flow、bounded outbox、redaction、metrics；不阻塞 egress stream |
| Phase 6 | `Canonical IR` source/sink adapter；Chat/Responses renderer；conversion notice 与 provider capability gate |

## 8. Provider conformance suite

每个 provider adapter 必须通过相同的基础测试包，并额外提供 provider-specific fixtures：

1. descriptor/capability 与 route config 一致；未知 provider kind 无法启用。
2. request mapper 产生正确 path/query/body，且不静默丢弃受支持字段。
3. header/auth adapter 只向配置允许的 endpoint 发送明确 header；secret 永不进入 log/error/audit。
4. response adapter 能处理分片 UTF-8、多 SSE event 同 chunk、多行 `data:`、unknown event、terminal event、EOF 和 cancellation。
5. error adapter 正确区分 pre-first-event retry 与已输出 stream 后不可重试。
6. capability adapter 在出站前拒绝 unsupported feature，而不是删除参数后继续调用。
7. benchmark 记录 allocations、p50/p95 first-byte/first-event overhead、每 event copy count 和 slow-consumer backpressure；不以“traits 比 JSON 快”作为未经测量的结论。

## 9. 采用与拒绝的方案

| 方案 | 结论 | 原因 |
|---|---|---|
| Rust trait adapters + typed dataflow | 采用 | provider 差异与安全关键 header 可编译、可审查、可测试；共享 transport 仍避免复制 |
| LiteLLM-style JSON Registry | 拒绝 | 把 provider 行为/安全 header/transform 放入运行时解释数据，难以给 Rust 类型系统和代码审查提供保证 |
| 单一巨型 `Provider` trait | 拒绝 | 所有 provider 被迫实现无关能力，产生 `Option`/provider-name branches，边界模糊 |
| 每个 provider 复制完整 HTTP/SSE client | 拒绝 | 重复 retry、SSE、cancellation、安全修复；应共享 transport，只适配差异 |
| 热路径 `Box<dyn Provider>` + string lookup | 初期拒绝 | 无必要动态性；闭合 enum 更简单，可在 route snapshot 边界后续替换 |
| 运行时 Rust plugin | 延期 | 提高供应链、ABI、配置与安全复杂度，当前 provider 数量和单凭证约束不需要 |

## 10. 验证与性能门

Rust 选择与 trait 拆分的性能收益必须以基准证明。Phase 0 建立 baseline；每新增 adapter 运行：

- `cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test`；
- SSE parser 的 allocation/copy microbenchmark；
- native forwarding 的 TTFT 与 steady-state event throughput benchmark；
- slow consumer 与 cancellation soak test；
- provider conformance suite；
- route config reload 与 in-flight snapshot consistency test；
- secret/redaction test，确保特定 header 和 credential 不进入日志/trace/audit。

性能优化的优先级是：避免无界 buffering、避免 token copy、正确 backpressure、连接复用与 cancellation，随后才是 enum/trait dispatch 的微优化。

## 11. 证据

- 用户确认：Rust 开发；provider 差异使用 trait 组合；避免 JSON Registry；整体采用数据流设计。
- [开发计划](../plans/development-plan.md)：阶段计划、OAuth 单 credential、协议和安全门。
- [架构与路线](architecture-and-roadmap.md)：数据面/控制面、SSE、路由和 quality gate。
- [Chat/Responses 转换设计](../design/chat-responses-conversion.md)：Canonical IR 与转换 stream state machine。
- [控制面、模型、密钥与可观测性](control-plane-models-keys-and-observability.md)：route snapshot、alias、key 和 audit 边界。
