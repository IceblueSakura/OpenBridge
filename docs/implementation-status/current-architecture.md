# 当前代码架构

## 状态与边界

**已实现事实。** 当前生产注册表使用
`ModelConfig`、`UpstreamTargetConfig`、`UpstreamApiConfig`、
`PublicModelConfig` 与 `RouteConfig`，请求路径使用 `RequestRequirements + RoutePlan`。
最近一次记录已完成格式化、全量 Rust 测试与 Clippy；需要下载外部 SDK 的兼容性测试仍保持 ignored，
真实 Provider、负载和长期运行验证未执行。

当前生产请求同时支持 Native Path 与显式 `Bridged` Route。请求级 `AttemptManager`、单进程跨请求
cooldown、`BridgePlan`、双向 JSON/SSE renderer 和 stream 状态机已经接入统一 ingress；模型信息扩展接口
尚未实现。

## 1. 分层结构

```text
bootstrap / users / process environment
          ↓
composition root
          ↓
immutable RuntimeRegistry + UserRegistry + CredentialStore
          ↓
HTTP ingress
          ↓
RequestRequirements → RoutePlan / optional BridgePlan
          ↓
ProviderAdapter + UpstreamTarget + UpstreamApi
          ↓
shared UpstreamTransport / SSE observation or bridge rendering
          ↓
upstream provider
```

依赖方向保持单向：配置和注册表不执行网络 I/O；pipeline 不按 Provider 名称分支；adapter 不选择
Public Model 或 Route；transport 不解释模型和协议能力。

### 1.1 关键代码词汇

| 层 | 核心类型 | 简单定义 |
|---|---|---|
| 启动配置 | `BootstrapConfig`、`RuntimeLimits`、`HttpClientConfig` | 进程启动参数、请求限制和 HTTP client 参数 |
| Credential | `CredentialId`、`CredentialStoreBuilder`、`CredentialStore`、`UpstreamCredential` | 启动时合并上下游 secret、隔离用途并提供只读借用视图 |
| 下游身份 | `UserConfigPath`、`UserConfiguration`、`UserRegistry`、`User` | 启动时分离用户元数据与 Key，通过 Store 匹配后提供稳定用户身份 |
| API 语义 | `ApiProtocol`、`ApiRequest`、`ApiCapabilities` | 下游协议、原始请求和协议能力 |
| 注册配置 | `ModelConfig`、`UpstreamTargetConfig`、`UpstreamApiConfig`、`RouteConfig`、`PublicModelConfig` | 编译期写入并等待校验的配置 |
| 运行注册表 | `RuntimeRegistry`、`ModelInfo`、`UpstreamTarget`、`UpstreamApi`、`Route`、`PublicModel` | 校验通过后供请求路径只读使用的数据 |
| 请求规划 | `RequestRequirements`、`RoutePlan`、`RouteCandidate` | 请求需要什么、可走哪些 route、每条 route 绑定到哪里 |
| Bridge | `BridgePlan`、`BridgeStreamRenderer`、`ChatStreamState`、`ResponsesStreamState` | 受限双向请求/响应转换及单请求 stream lifecycle、tool identity 与 arguments 重建 |
| Provider | `ProviderContract`、`ProviderAdapter`、`PreparedUpstreamRequest` | Provider 能力上界、闭合实现分派和待发送请求 |
| Transport | `UpstreamTransport`、`UpstreamClient`、`UpstreamResponse` | 可替换的发送边界、生产 HTTP client 和上游响应 |
| Probe | `ProbeOptions`、`ProbeResult`、`TargetProbeReport` | 探测输入、单项观察和 target 汇总报告 |

命名规则保持简单：`*Config` 表示构建前配置，去掉 `Config` 表示校验后的运行实体，`*Info` 表示只读事实，
`*Result` 表示一次操作结果，`*Error` 表示失败。`UpstreamTransport` 是当前唯一公开 trait，只用于隔离真实
HTTP 发送与可控 transport；Provider 的请求、认证、SSE 和错误处理统一由闭合 `ProviderAdapter` 直接分派，
不再拆成多组单方法 trait。

## 2. 装配与配置层

实现位置：`src/main.rs`、`src/config/*`、`src/providers/mod.rs`。

启动顺序：

```text
load_optional_dotenv
→ BootstrapConfigPath::load
→ UserConfigPath::load
→ providers::build_compiled_registry
→ CredentialStoreBuilder::load_upstream_environment
→ immutable CredentialStore
→ UpstreamClient::new
→ GatewayState::new
→ ingress::build_router
→ axum::serve
```

`bootstrap.toml` 拥有 loopback listener、私有用户文件位置、请求/SSE 大小和 HTTP client 参数。用户文件、
Provider、模型、target、upstream API、route、endpoint 和 credential locator 都只在启动阶段加载；没有 route TOML、
动态 Provider DSL 或热重载。`UserConfiguration` 把用户元数据交给 `UserRegistry`、把 Key 交给
`CredentialStoreBuilder`；composition root 再为所有启用 target 解析环境变量，缺失或空值会在 listener 绑定前
失败。构造后的 Store 是上下游 secret 的唯一运行时所有者，请求路径不再读取 credential 来源。

## 3. 注册表层

实现位置：`src/registry/mod.rs`、`src/models/*`、`src/providers/*`。

```text
RegistryConfig
  models: ModelConfig[]
  upstream_targets: UpstreamTargetConfig[]
    upstream_apis: UpstreamApiConfig[]
  routes: RouteConfig[]
  public_models: PublicModelConfig[]
```

各实体职责：

| 实体 | 所有内容 |
|---|---|
| `ProviderContract` | 代码拥有的 adapter、endpoint profile、credential kind 与能力上界 |
| `ModelConfig` | 与供应商无关的模型事实、context、参数与 reasoning 元数据 |
| `UpstreamTargetConfig` | Provider Family、Model、endpoint、credential、timeout、启停及 quota/fault 边界 |
| `UpstreamApiConfig` | 单一原生协议的 upstream model、served limits、能力证据、transport 与 state affinity |
| `RouteConfig` | target、upstream API、下游协议和 `Native`/`Bridged` 执行模式 |
| `PublicModelConfig` | 下游稳定模型名与有序完整 Route ID |

同一 target 可以同时注册 Chat 和 Responses Upstream API；二者可拥有不同 upstream model、context/output
限制、能力证据和 state affinity。共享 endpoint、credential、Model 与故障边界属于 target。

`build_registry` 验证引用、唯一性、credential、HTTPS endpoint、timeout、Provider 上界、Upstream API
协议/能力一致性、model rules 只收窄、Native/Bridged route 协议方向及 Public Model route 顺序。成功后生成：

```text
RuntimeRegistry
  models
  upstream_targets → upstream_apis
  routes
  public_models
```

## 4. HTTP 接入层

实现位置：`src/ingress/*`。

| Endpoint | 当前处理 |
|---|---|
| `GET /healthz` | 返回状态与注册表版本 |
| `GET /v1/models` | 枚举 Public Model 名称 |
| `POST /v1/chat/completions` | 进入 Chat Native/Bridged RoutePlan |
| `POST /v1/responses` | 进入 Responses Native/Bridged RoutePlan |

Ingress 执行认证、body/content-type 限制、本地错误归一化和当前的首输出前 attempt 循环。它不接受
客户端提供的上游 URL、credential 或内部 route ID。

## 5. 请求分析与路由层

实现位置：`src/core/*`、`src/pipeline/mod.rs`。

```text
raw body + downstream protocol
→ analyze_request
→ RequestRequirements
→ Public Model ordered Routes
→ protocol / capability / limit / reasoning gates
→ RoutePlan<RouteCandidate>
```

`RequestRequirements` 只记录请求事实：public model、协议、streaming、功能组合、输出限制和状态亲和指示。
`RoutePlan` 固定有序的 Route、Upstream Target 与 Upstream API。Native candidate 保留原始 `ApiRequest`；
Bridged candidate 在 egress 前生成受限 `BridgePlan` 与相反协议的 `ApiRequest`。Provider adapter 仍只负责
目标 endpoint、真实 model、header 与认证改写。

请求携带 `previous_response_id` 时，计划关闭跨 target fallback。不同 route 或不同 Upstream API 的能力不会
按字段求并集；一条候选必须独立满足完整请求。

## 6. Provider 适配层

实现位置：`src/provider/*`、`src/providers/openai.rs`、`src/providers/longcat.rs`。

`ProviderKind` 是闭合集合。具体 adapter 从 selected Upstream API 读取 upstream model，负责相对 path、模型
字段改写、受信 request-header hook、认证 header、响应/SSE terminal 和错误分类。当前 OpenAI 与 LongCat
hook 只从下游选择 `User-Agent` 写入 `SafeHeaders`；credential header 在 hook 之后独立附加。credential
locator 与 endpoint/timeout 则来自 selected Upstream Target。Ingress 按完整 `binding_id + ProviderKind` 从
`CredentialStore` 借用 `UpstreamCredential`；Store 不公开通用明文查询，adapter 仍在 crate 内的认证 header
边界才访问 secret。

OpenAI 与 LongCat 当前都注册 Chat、Responses 两个独立 Upstream API；每个下游协议先列 Native route，
再列指向相反 Upstream API 的 Bridged route。Bridge 生产路径由编译注册表、记录型 transport 与 canonical
wire 确定性验证，但尚未调用真实异构协议 Provider。

## 7. Transport、SSE、attempt 与 health

实现位置：`src/transport/*` 与 `ingress::forward_request`。

共享 `UpstreamClient` 只接收已解析 target 和 adapter 生成的相对 URI，禁止 redirect，并应用 target
timeout。Native streaming response 保持业务 bytes 透明并由 `SseDecoder` 观察 framing/terminal；Bridged
stream 则按完整 event 增量渲染目标协议 wire。下游丢弃任一 body 时，上游 stream 随之取消。

`ingress::attempt::AttemptManager` 管理单请求 attempt 生命周期：stream/non-stream 共享最多 6 次的硬预算，
每候选最多 2 次，attempt 间从 50 ms 起按二倍增长并 capped 到 500 ms；在预算可容纳时为未尝试候选保留
机会。RoutePlan 允许时可进入同一 Public Model 的下一完整候选；下游取消会销毁 pending send、timer 或
response body，提交 response 后不得再拼接另一上游响应。

`ingress::health::TargetHealth` 在所有 `GatewayState` clone 间共享。它只接受注册表提供的 quota/fault scope，
把 429 隔离到 `quota_scope`，把暂时性 5xx/transport failure 隔离到 `fault_domain`；默认 cooldown 为 1 秒，
`Retry-After` 最长采用 30 秒。无状态请求跳过冷却 scope，target-bound continuation 忽略 cooldown 并保持原
target 亲和。该状态不持久化、不跨进程，也不执行动态权重或 credential 轮换。

`src/bridge.rs` 与 `src/bridge/conversion.rs` 实现生产 Protocol Bridge。Responses 侧分别固定 response id、
item id、call id 和 output index；Chat 侧只用 tool index 关联同一 stream 的分片，不用它替代 call id。两侧
要求唯一 terminal 和闭合 JSON object arguments。`BridgePlan` 只接受显式 allowlist 内的共同 text/function
语义；无法表达的字段、opaque continuation 与私有扩展在 egress 前拒绝。

## 8. Probe 与验证层

`openbridge-probe --target <id>` 针对固定 Upstream Target 工作，并按协议选择对应 Upstream API。它复用
target endpoint、adapter 与 transport，只为管理员选中的 target 构造一个上游 `CredentialStore` 快照；它不
加载下游用户 Key、不接受 URL/model/header/credential 覆盖，也不修改 `RuntimeRegistry`。

测试夹具使用 target/upstream API/route 和 `RequestRequirements + RoutePlan` API。2026-08-01 最近一次执行
`cargo test --locked`，91 个测试通过、1 个外部 SDK 集成测试 ignored；
`cargo clippy --locked -- -D warnings` 通过。未执行外部 SDK、独立 Python/curl 黑盒测试、目标 Agent、
真实 Provider、负载或长期运行验证。

## 9. 尚未实现

- 动态 Converter catalog、route-local 可配置 ConversionPolicy 与异构 Provider 实测；
- 动态 availability/weight、持久化或分布式 cooldown；
- 可安全投影真实 route/upstream API 信息的内部视图与任何扩展 HTTP API；
- Responses WebSocket、OAuth、hosted tool、MCP 和动态 Provider/plugin DSL。

## 关联文档

- [当前实现说明](current-implementation.md)
- [配置、凭证与受信边界](../functional-requirements/configuration-and-credentials.md)
- [路由与 Provider 韧性](../functional-requirements/provider-resilience.md)
