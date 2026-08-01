# 当前代码架构

## 状态与边界

**已实现事实。** 当前生产注册表使用
`ModelConfig`、`UpstreamTargetConfig`、`UpstreamApiConfig`、
`PublicModelConfig` 与 `RouteConfig`，请求路径使用 `RequestRequirements + RoutePlan`。
最近一次记录只包含格式化与编译检查，没有运行测试，因此这里不把既有行为描述为已测试验收。

当前只有 Native Path。Protocol Bridge、统一 `AttemptManager`、跨请求 cooldown 和模型信息扩展
接口尚未实现。

## 1. 分层结构

```text
bootstrap / process environment
          ↓
composition root
          ↓
immutable RuntimeRegistry
          ↓
HTTP ingress
          ↓
RequestRequirements → RoutePlan
          ↓
ProviderAdapter + UpstreamTarget + UpstreamApi
          ↓
shared UpstreamTransport / SSE observation
          ↓
upstream provider
```

依赖方向保持单向：配置和注册表不执行网络 I/O；pipeline 不按 Provider 名称分支；adapter 不选择
Public Model 或 Route；transport 不解释模型和协议能力。

### 1.1 关键代码词汇

| 层 | 核心类型 | 简单定义 |
|---|---|---|
| 启动配置 | `BootstrapConfig`、`RuntimeLimits`、`HttpClientConfig` | 进程启动参数、请求限制和 HTTP client 参数 |
| API 语义 | `ApiProtocol`、`ApiRequest`、`ApiCapabilities` | 下游协议、原始请求和协议能力 |
| 注册配置 | `ModelConfig`、`UpstreamTargetConfig`、`UpstreamApiConfig`、`RouteConfig`、`PublicModelConfig` | 编译期写入并等待校验的配置 |
| 运行注册表 | `RuntimeRegistry`、`ModelInfo`、`UpstreamTarget`、`UpstreamApi`、`Route`、`PublicModel` | 校验通过后供请求路径只读使用的数据 |
| 请求规划 | `RequestRequirements`、`RoutePlan`、`RouteCandidate` | 请求需要什么、可走哪些 route、每条 route 绑定到哪里 |
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
→ providers::build_compiled_registry
→ UpstreamClient::new
→ GatewayState::with_environment_credentials
→ ingress::build_router
→ axum::serve
```

`bootstrap.toml` 只拥有 loopback listener、请求/SSE 大小和 HTTP client 资源策略。Provider、模型、
target、upstream API、route、endpoint 和 credential locator 均由 Rust 代码显式注册；没有 route TOML、
动态 Provider DSL 或热重载。

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
| `RouteConfig` | target、upstream API、下游协议和当前 `Native` 执行模式 |
| `PublicModelConfig` | 下游稳定模型名与有序完整 Route ID |

同一 target 可以同时注册 Chat 和 Responses Upstream API；二者可拥有不同 upstream model、context/output
限制、能力证据和 state affinity。共享 endpoint、credential、Model 与故障边界属于 target。

`build_registry` 验证引用、唯一性、credential、HTTPS endpoint、timeout、Provider 上界、Upstream API
协议/能力一致性、model rules 只收窄、Native route 协议方向及 Public Model route 顺序。成功后生成：

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
| `POST /v1/chat/completions` | 进入 Chat Native Path |
| `POST /v1/responses` | 进入 Responses Native Path |

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
`RoutePlan` 固定有序的 Route、Upstream Target、Upstream API 与原始 `ApiRequest`。
它不执行协议转换或 adapter 字段改写。

请求携带 `previous_response_id` 时，计划关闭跨 target fallback。不同 route 或不同 Upstream API 的能力不会
按字段求并集；一条候选必须独立满足完整请求。

## 6. Provider 适配层

实现位置：`src/provider/*`、`src/providers/openai.rs`、`src/providers/longcat.rs`。

`ProviderKind` 是闭合集合。具体 adapter 从 selected Upstream API 读取 upstream model，负责相对 path、模型
字段改写、认证 header、响应/SSE terminal 和错误分类。credential locator 与 endpoint/timeout 则来自
selected Upstream Target。

OpenAI 与 LongCat 当前都注册 Chat、Responses 两个独立 Upstream API，wire 仍均为
OpenAI-compatible；这不构成异构协议桥已实现的证据。

## 7. Transport、SSE 与 attempt

实现位置：`src/transport/*` 与 `ingress::forward_native`。

共享 `UpstreamClient` 只接收已解析 target 和 adapter 生成的相对 URI，禁止 redirect，并应用 target
timeout。Streaming response 保持业务 bytes 透明；`SseDecoder` 只观察 UTF-8、framing、event size 和
terminal。下游丢弃 body 时，上游 stream 随之取消。

当前 retry/fallback 仍位于 Ingress，而非独立 `AttemptManager`：仅 streaming 请求能在首个下游 body
之前进行固定次数 retry，并在 RoutePlan 允许时进入下一候选；首输出后不得拼接另一上游响应。

## 8. Probe 与验证层

`openbridge-probe --target <id>` 针对固定 Upstream Target 工作，并按协议选择对应 Upstream API。它复用
target endpoint、credential、adapter 与 transport，不接受 URL/model/header 覆盖，不修改 `RuntimeRegistry`。

测试夹具使用 target/upstream API/route 和 `RequestRequirements + RoutePlan` API。最近一次只确认所有 target
能够编译，没有执行测试用例、Clippy、真实 Provider 或 SDK 验证。

## 9. 尚未实现

- Converter catalog、route-local ConversionPolicy、BridgePlan 与 Chat/Responses Bridge；
- 独立 AttemptManager、统一 unsupported/fallback/availability 与跨请求 cooldown；
- 可安全投影真实 route/upstream API 信息的内部视图与任何扩展 HTTP API；
- Responses WebSocket、OAuth、hosted tool、MCP 和动态 Provider/plugin DSL。

## 关联文档

- [当前实现说明](current-implementation.md)
- [配置、凭证与受信边界](../functional-requirements/configuration-and-credentials.md)
- [路由与 Provider 韧性](../functional-requirements/provider-resilience.md)
