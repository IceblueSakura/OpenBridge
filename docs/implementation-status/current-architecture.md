# 当前代码架构

## 状态与边界

**已实现事实。** 当前源码已经完成架构迁移总计划 M1–M4 的结构切换：生产注册表使用
`RealModelDefinition`、`UpstreamTargetDefinition`、`NativeOfferingDefinition`、
`PublicModelDefinition` 与 `ServingRouteDefinition`，请求路径使用 `RequestProfile + RoutePlan`。
本次只做了格式化与编译检查，没有运行测试，因此这里不把既有行为描述为已重新测试验收。

当前仍只有 Native Path。Protocol Bridge、统一 `AttemptManager`、跨请求 cooldown 和模型信息扩展
接口尚未实现，分别保留在总计划 M5–M7。

## 1. 分层结构

```text
bootstrap / process environment
          ↓
composition root
          ↓
immutable RegistrySnapshot
          ↓
HTTP ingress
          ↓
RequestProfile → RoutePlan
          ↓
ProviderAdapter + UpstreamTarget + NativeOffering
          ↓
shared UpstreamTransport / SSE observation
          ↓
upstream provider
```

依赖方向保持单向：配置和注册表不执行网络 I/O；pipeline 不按 Provider 名称分支；adapter 不选择
Public Model 或 Serving Route；transport 不解释模型和协议能力。

## 2. 装配与配置层

实现位置：`src/main.rs`、`src/config/*`、`src/providers/mod.rs`。

启动顺序：

```text
load_optional_dotenv
→ BootstrapPath::load
→ providers::build_compiled_registry
→ UpstreamClient::new
→ AppState::with_environment_credentials
→ ingress::build_router
→ axum::serve
```

`bootstrap.toml` 只拥有 loopback listener、请求/SSE 大小和 HTTP client 资源策略。Provider、模型、
target、offering、route、endpoint 和 credential locator 均由 Rust 代码显式注册；没有 route TOML、
动态 Provider DSL 或热重载。

## 3. 注册表层

实现位置：`src/registry/mod.rs`、`src/models/*`、`src/providers/*`。

```text
RegistryDefinition
  real_models: RealModelDefinition[]
  upstream_targets: UpstreamTargetDefinition[]
    offerings: NativeOfferingDefinition[]
  serving_routes: ServingRouteDefinition[]
  public_models: PublicModelDefinition[]
```

各实体职责：

| 实体 | 所有内容 |
|---|---|
| `ProviderDescriptor` | 代码拥有的 adapter、endpoint profile、credential kind 与能力上界 |
| `RealModelDefinition` | 与供应商无关的模型事实、context、参数与 reasoning 元数据 |
| `UpstreamTargetDefinition` | Provider Family、Real Model、endpoint、credential、timeout、启停及 quota/fault 边界 |
| `NativeOfferingDefinition` | 单一原生协议的 upstream model、served limits、能力证据、transport 与 state policy |
| `ServingRouteDefinition` | target、offering、下游协议和当前 `Native` 执行模式 |
| `PublicModelDefinition` | 下游稳定模型名与有序完整 Serving Route ID |

同一 target 可以同时注册 Chat 和 Responses Offering；二者可拥有不同 upstream model、context/output
限制、能力证据和 state policy。共享 endpoint、credential、Real Model 与故障边界属于 target。

`build_registry` 验证引用、唯一性、credential、HTTPS endpoint、timeout、Provider 上界、Offering
协议/能力一致性、模型约束只收窄、Native route 协议方向及 Public Model route 顺序。成功后生成：

```text
RegistrySnapshot
  real_models
  upstream_targets → resolved offerings
  serving_routes
  public_models
```

旧 `ProviderDefinition`、`DeploymentDefinition`、`AliasDefinition` 及其 resolved 类型已从生产代码删除。

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
→ RequestProfile
→ Public Model ordered Serving Routes
→ protocol / capability / limit / reasoning gates
→ RoutePlan<ExecutionPlanCandidate>
```

`RequestProfile` 只记录请求事实：public model、协议、streaming、功能组合、输出限制和状态亲和指示。
`RoutePlan` 固定有序的 Serving Route、Upstream Target、Native Offering 与原始 `ValidatedRequest`。
它不执行协议转换或 adapter 字段改写。

请求携带 `previous_response_id` 时，计划关闭跨 target fallback。不同 route 或不同 Offering 的能力不会
按字段求并集；一条候选必须独立满足完整请求。

## 6. Provider 适配层

实现位置：`src/provider/*`、`src/providers/openai.rs`、`src/providers/meituan.rs`。

`ProviderKind` 是闭合集合。具体 adapter 从 selected Offering 读取 upstream model，负责相对 path、模型
字段改写、认证 header、响应/SSE terminal 和错误分类。credential locator 与 endpoint/timeout 则来自
selected Upstream Target。

OpenAI 与 Meituan/LongCat 当前都注册 Chat、Responses 两个独立 Offering，wire 仍均为
OpenAI-compatible；这不构成异构协议桥已实现的证据。

## 7. Transport、SSE 与 attempt

实现位置：`src/transport/*` 与 `ingress::forward_native`。

共享 `UpstreamClient` 只接收已解析 target 和 adapter 生成的相对 URI，禁止 redirect，并应用 target
timeout。Streaming response 保持业务 bytes 透明；`SseDecoder` 只观察 UTF-8、framing、event size 和
terminal。下游丢弃 body 时，上游 stream 随之取消。

当前 retry/fallback 仍位于 Ingress，而非独立 `AttemptManager`：仅 streaming 请求能在首个下游 body
之前进行固定次数 retry，并在 RoutePlan 允许时进入下一候选；首输出后不得拼接另一上游响应。

## 8. Probe 与验证层

`openbridge-probe --target <id>` 针对固定 Upstream Target 工作，并按协议选择对应 Offering。它复用
target endpoint、credential、adapter 与 transport，不接受 URL/model/header 覆盖，不修改 snapshot。

测试夹具已迁移到 target/offering/route 和 `RequestProfile + RoutePlan` API。本次迁移只确认所有 target
能够编译，没有执行测试用例、Clippy、真实 Provider 或 SDK 验证。

## 9. 尚未实现

- M5：Converter catalog、route-local ConversionPolicy、ResolvedBridgePlan 与 Chat/Responses Bridge；
- M6：独立 AttemptManager、统一 unsupported/fallback/availability 与跨请求 cooldown；
- M7：可安全投影真实 route/offering 信息的内部视图与任何扩展 HTTP API；
- Responses WebSocket、OAuth、hosted tool、MCP 和动态 Provider/plugin DSL。

## 关联文档

- [当前实现说明](current-implementation.md)
- [架构迁移总计划](../implementation-plans/registry-architecture-migration.md)
- [代码注册表与原生路由](../implementation-plans/configuration-and-routing.md)
- [Provider adapter 与数据流](../implementation-plans/provider-adapters-and-dataflow.md)
- [目标服务架构](../implementation-plans/service-architecture.md)
