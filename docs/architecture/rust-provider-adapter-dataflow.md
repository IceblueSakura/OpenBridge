# Rust Provider adapter 与数据流架构

## 状态

**Working hypothesis；原型部分验证。**

当前代码已经验证 typed route snapshot、单一 OpenAI adapter、共享 HTTP transport、SSE framing、原生 Chat/Responses pipeline、capability gate 和输出前 fallback。当前 transport 范围是 HTTP JSON/SSE，不包含 Responses WebSocket。它尚未证明 Provider Family 边界、异构协议 adapter、bridge IR 或最终配置模型已经收敛。

## 1. 当前首选方向

系统使用 Rust，并采用：

> **编译期 Provider Family + 受信运行时 Deployment 配置 + 有类型异步数据流**

这比原先“每个 Provider 都是编译期闭合 catalog entry”的表述更精确：

- 协议、认证、必要 header、错误、响应/SSE 和转换行为由 Rust adapter 实现；
- 服务所有者可以在配置中创建多个 deployment，提供 base URL、credential reference、上游模型和能力；
- 同一 OpenAI-compatible Provider Family 不应因为 host/model 不同而复制 Rust 实现；
- 下游业务请求不能提交任意 URL、credential、认证 header 或 transform；
- 不用运行时 JSON 模板或脚本解释安全关键 Provider 行为。

该方向仍需用至少一个 OpenAI Responses、一个 Generic Chat 和一个 Anthropic Messages archetype 反证。

## 2. 目标与非目标

### 目标

- 为异构 Provider 提供有边界、可编译、可 fixture-test 的 adapter 扩展点；
- 让 Provider-specific auth/header/path/error 在代码审查和类型检查中可见；
- 让 native request 保留未知合法字段，避免通用 IR 成为兼容性瓶颈；
- 让 bridge request 使用明确的协议对转换器，而不是散布 provider-name if/else；
- 共享 HTTP、SSE、retry boundary、cancellation 和 backpressure；
- 在单用户部署下保持配置和模块足够简单。

### 非目标

- 运行时加载 Rust plugin、动态库或第三方脚本；
- 通过业务请求注入上游 host、header 或 credential；
- 同 Provider 多账号池和 credential selector；
- 为多租户授权、配额或合规审计提前污染 Provider trait；
- 将每个 Provider 能力塞入一个巨型 trait；
- 为每个 Provider 复制完整 HTTP/SSE client。

## 3. 模块边界

项目早期保持单 Cargo crate：

```text
src/
  core/           # IDs, RoutePlan, capability, ProxyError
  config/         # trusted owner config, validation, immutable snapshots
  ingress/        # Chat/Responses HTTP JSON/SSE handlers and static token
  routing/        # alias, eligibility, selection, attempt policy
  protocol/       # wire types and Bridge IR
  bridge/         # protocol-pair request/response/stream conversion
  provider/       # ProviderFamily catalog and adapter traits
  providers/      # concrete family implementations
  transport/      # shared reqwest client, SSE framing, cancellation
  usage/          # optional bounded UsageRecord sink
```

建议依赖方向：

```text
core/protocol
    ↓
routing + bridge contracts
    ↓
provider traits
    ↓
provider implementations
    ↓
shared transport

config/ingress assemble these components but do not own provider behavior.
```

`ingress` 不识别 Provider token/header 形状；`routing` 不解析 Provider SSE；`providers/*` 不决定 public alias。

## 4. Provider Family 与 Deployment

### 4.1 Provider Family

建议使用闭合 enum 或等价 typed registry 表示已编译协议族：

```rust
pub enum ProviderFamily {
    OpenAi,
    OpenAiCompatible,
    Anthropic,
}
```

初期 enum dispatch 足够简单。未来若 feature-gated build 或依赖隔离确有需要，可在 snapshot 构建边界替换为 trait object，但不应在每个 token/event 上做字符串 registry lookup。

### 4.2 Deployment

运行时配置实例：

```rust
pub struct DeploymentConfig {
    pub id: DeploymentId,
    pub family: ProviderFamily,
    pub base_url: Url,
    pub credential: SecretReference,
    pub upstream_model: String,
    pub native_protocols: ProtocolSet,
    pub native_transports: TransportSet,
    pub capabilities: CapabilityProfile,
    pub timeout: Duration,
    pub enabled: bool,
}
```

Provider Family 定义 capability 上界；deployment 配置可以选择或收窄已支持能力。若需要扩大能力，必须有 adapter 支持和 fixture，不允许仅靠配置声明。

## 5. Adapter 职责

避免一个包含所有可能性的巨型 `Provider` trait。建议按职责拆分：

| Trait / role | 输入 → 输出 | 职责 |
|---|---|---|
| `ProviderDescriptor` | static metadata | family id、native protocols/transports、配置 schema、capability upper bound |
| `NativeRequestAdapter` | original JSON + RoutePlan → upstream request | 最小改写 path/query/model/body；保留未知合法字段 |
| `BridgeRequestAdapter` | Bridge IR + RoutePlan → upstream request | 仅用于跨协议路径 |
| `AuthAdapter` | credential lease + deployment → headers | 发送前构造认证，不暴露 secret |
| `HeaderAdapter` | deployment/request metadata → safe headers | adapter 明确允许的非认证 header |
| `NativeResponseAdapter` | upstream response/SSE → downstream wire/outcome | 原生协议 terminal/error/header 规则 |
| `BridgeResponseAdapter` | upstream response/SSE → Bridge IR events | 跨协议解析和 identity/state assembly |
| `ErrorAdapter` | upstream failure → error class | retryability、safe envelope、attempt boundary |
| `CapabilityAdapter` | request features + deployment + bridge catalog → decision | 从原生 profile 推导 `Native/Bridged/Unsupported/Unknown` |

同协议请求不应被迫使用 `BridgeRequestAdapter`/`BridgeResponseAdapter`。

## 6. Typed dataflow

### 6.1 Native Path

```text
InboundHttpRequest
→ ValidatedRequest
→ ClassifiedRequest
→ RoutePlan(mode=native)
→ NativeProviderRequest
→ UpstreamHttpResponse / UpstreamByteStream
→ NativeClientResponse / NativeClientSseStream
```

Native Path 可以解析少量 feature 字段用于 capability gate，但应保留原始 JSON value/bytes 作为转发主体。未知字段不能因内部 struct 缺失而自动丢弃。

### 6.2 Bridge Path

```text
InboundHttpRequest
→ ValidatedRequest
→ ClassifiedRequest
→ RoutePlan(mode=bridge)
→ SourceProtocolParser
→ BridgeRequest
→ TargetProviderRequest
→ UpstreamResponse / EventStream
→ TargetStreamAssembler
→ BridgeResponse/Event
→ SourceProtocolRenderer
```

bridge 的 state 和 identity 明确放在 request/stream-scoped context；不能依赖全局 provider-name cache 猜测恢复。

### 6.3 共同约束

每个 stage：

- 返回明确 envelope 或 `ProxyError`；
- 传播 request deadline 和 cancellation；
- 使用 pull-based stream 或有界 channel；
- 只在跨异步边界确有共享时使用 `Arc`；
- byte payload 使用 `Bytes`/borrowed view，避免逐 token 拷贝；
- 持有 immutable RoutePlan 与 config snapshot；
- 不逐 token 同步写文件或 SQLite。

SSE framing 属于 transport；协议层只消费完整 event，不能把 HTTP chunk 当 event。

## 7. 运行时配置边界

### 允许的受信配置数据

```text
provider family
base URL / origin
credential secret reference
upstream model
public alias / candidate mapping
timeout / enable state
native protocol declarations
capability declarations or narrowing overrides
adapter 明确允许的非认证 header values
```

### 禁止的运行时行为注入

```text
arbitrary auth scheme
arbitrary request/response template
unrestricted header forwarding
client-supplied base URL
arbitrary retry/error classification
runtime script / dynamic library
```

“受信服务所有者可配置 endpoint”与“业务请求不可指定 endpoint”必须明确区分。对本地 vLLM、兼容网关或自建 endpoint，前者是核心需求；后者是安全不变量。

## 8. Capability Profile

deployment/model 的原生 `CapabilityProfile` 先记录：

```rust
pub enum NativeSupport {
    Supported,
    Unsupported,
    Unknown,
}
```

route planner 再结合下游协议、下游/上游 transport、Bridge catalog 和请求 feature 生成：

```rust
pub enum SupportLevel {
    Native,
    Bridged,
    Unsupported,
    Unknown,
}
```

`Bridged` 不能仅由配置扩大；它必须有已实现 converter 和 fixture 证据。`CapabilityProfile` 至少包含：

```text
responses
chat_completions
responses_websocket
streaming
function_tools
parallel_tools
structured_output
reasoning
multimodal_input
hosted_tools
continuation
usage_streaming
```

组合 feature 必须可被 request classifier 表达。例如 `streaming + parallel_tools` 不能仅由两个孤立布尔值自动推导；若尚无组合证据，保持 `Unknown` 或在 adapter 中增加条件判断。

## 9. Route planner 接口

Provider adapter 不决定 candidate 顺序。route planner 输出：

```rust
pub struct RoutePlan {
    pub alias: ModelAlias,
    pub selected: DeploymentSnapshot,
    pub remaining_candidates: Vec<DeploymentSnapshot>,
    pub downstream_protocol: Protocol,
    pub downstream_transport: Transport,
    pub upstream_protocol: Protocol,
    pub upstream_transport: Transport,
    pub mode: RouteMode,
    pub capability: CapabilityDecision,
    pub continuation: Option<ContinuationBinding>,
    pub fallback: FallbackBoundary,
    pub config_version: ConfigVersion,
}
```

请求一旦开始，selected deployment 和 credential binding 不随 reload 改变。

## 10. Provider conformance suite

每个 Provider Family 至少覆盖：

1. 配置 schema、native protocol/transport 和 capability upper bound；
2. 正确 path/query/model/body；
3. 认证与必要 header；
4. secret 不进入 log/error/fixture；
5. JSON 非流式成功与错误；
6. SSE fragmented UTF-8、多 event、跨 chunk、多行 `data:`、unknown event；
7. terminal、EOF、cancel、idle timeout；
8. function tool schema/call/result；
9. Provider-specific error 和 retry classification；
10. client disconnect 时关闭上游连接；
11. Native Path 未知字段保留；
12. bridge 不支持能力在调用前拒绝。

除 mock fixture 外，至少保留一组真实或脱敏 Provider corpus。

## 11. 性能与资源验证

优先验证：

- 无无界 buffering；
- client disconnect 取消传播；
- 连接复用；
- Native Path 不做完整 IR parse/render；
- 每 event allocation/copy；
- slow consumer 下的内存上限；
- proxy TTFT overhead；
- bridge assembler 在超限 arguments/item 下 fail closed。

enum/trait dispatch 微优化不是当前首要问题，必须以 benchmark 而不是直觉决定。

## 12. 当前待证假设

| 假设 | 需要的反证/实验 |
|---|---|
| 编译期 Provider Family 足以表达安全关键行为 | 用 OpenAI Responses、Generic Chat、Anthropic Messages 三类 adapter 建模。 |
| 运行时 Deployment 能覆盖自定义兼容 endpoint | 接入本地 vLLM/第三方兼容 endpoint，验证 base URL/header 差异。 |
| Native Path 绕过 IR 更兼容且更轻 | 对比完整 parse/render 的字段保留、分配和 TTFT。 |
| 四态 capability 足够 | 收集组合 feature 和模型级差异，确认是否需要条件表达式。 |
| request-scoped bridge context 足够 | 用多轮 tool loop 和 `previous_response_id` 判断是否需要受限 ledger。 |

## 13. 采用、拒绝与延期

| 方案 | 状态 | 原因 |
|---|---|---|
| Typed Provider Family + runtime Deployment | Working hypothesis | 兼顾代码可审查性和单用户自定义 endpoint。 |
| Native Path 绕过 Bridge IR | Working hypothesis | 避免字段损失和不必要 parse/render。 |
| 完全 JSON Registry 定义 Provider 行为 | Rejected for core | auth/header/transform 难以通过 Rust 类型和代码审查约束。 |
| 每个 endpoint 一个编译期 Provider variant | Rejected | 对兼容 endpoint 过度刚性。 |
| 单一巨型 Provider trait | Rejected | 大量无关 `Option` 和 provider-name branch。 |
| 每个 Provider 复制 transport | Rejected | 重复 SSE、取消、retry 和安全修复。 |
| runtime native plugin | Deferred | 当前 Provider 数量和单用户范围不需要。 |

## 14. 关联文档

- [核心需求](../requirements/proxy-requirements.md)
- [目标架构与路线](architecture-and-roadmap.md)
- [本地配置、路由与使用量](local-configuration-routing-and-usage.md)
- [Chat/Responses bridge](../design/chat-responses-conversion.md)
- [当前实现说明](../implementation/current-implementation.md)
