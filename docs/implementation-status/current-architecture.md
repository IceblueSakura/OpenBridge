# 当前代码架构

## 状态与文档边界

**已实现事实。** 本文按运行时层次描述当前源码中的模块、依赖方向和请求路径。文中的 `ModelDefinition`、`DeploymentDefinition`、`AliasDefinition` 等名称均是当前 Rust 类型，不代表目标注册表已经采用这些概念。

目标架构见[目标服务架构](../implementation-plans/service-architecture.md)，从当前类型迁移到 `RealModel`、`UpstreamTarget`、`NativeOffering`、`PublicModel` 和 `ServingRoute` 的步骤见[注册表与路由架构迁移计划](../implementation-plans/registry-architecture-migration.md)。

## 1. 总体结构

OpenBridge 当前是单进程 Axum 服务。启动阶段一次性装载 bootstrap、构建代码注册表与共享 HTTP client；请求路径只读取不可变 `RegistrySnapshot`。

```text
config/bootstrap.toml + process environment
                    │
                    ▼
       startup and composition root
                    │
                    ▼
          immutable RegistrySnapshot
                    │
Client → HTTP ingress → native request pipeline
                    │
                    ▼
             ProviderAdapter
                    │
                    ▼
       shared UpstreamTransport / SSE
                    │
                    ▼
             upstream provider
```

当前只有 Native Path：Chat 请求只发往声明支持 Chat 的候选，Responses 请求只发往声明支持 Responses 的候选。代码没有 Chat ↔ Responses converter、Bridge IR、ServingRoute 或独立 ExecutionPlan。

## 2. 运行与装配层

实现位置：

- `src/main.rs`
- `src/config/*`
- `src/providers/mod.rs`

启动顺序：

```text
load_optional_dotenv
→ BootstrapPath::load
→ providers::build_compiled_registry
→ UpstreamClient::new
→ AppState::with_environment_credentials
→ ingress::build_router
→ TcpListener / axum::serve
```

该层负责：

- 装载 loopback listener、request/SSE 大小限制和共享 HTTP client 策略；
- 调用显式代码注册入口并在监听前完成注册表校验；
- 创建共享 `RegistrySnapshot`、`UpstreamTransport`、下游静态 Bearer credential 和上游 `CredentialSource`；
- 安装 Ctrl+C graceful shutdown。

该层不负责业务路由，也不接受运行时 Provider DSL、route TOML 或客户端提供的上游 URL。

## 3. 注册表层

实现位置：

- `src/registry/mod.rs`
- `src/models/*`
- `src/providers/*`

### 3.1 当前定义模型

```text
RegistryDefinition
  models: ModelDefinition[]
  providers: ProviderDefinition[]
  deployments: DeploymentDefinition[]
  aliases: AliasDefinition[]
```

当前职责实际分布如下：

| 当前类型 | 当前承载内容 | 已知结构限制 |
|---|---|---|
| `ModelDefinition` | 模型展示信息、context、参数、reasoning 事实 | 不能区分模型内在事实和供应商/协议证据 |
| `ProviderDefinition` | `ProviderKind` 与一个 credential binding | 把 adapter 类型和 credential 所有权绑在一起 |
| `DeploymentDefinition` | Provider/Model 引用、endpoint、upstream model、timeout、两种协议能力 | 同一对象同时承担调用边界和协议级供应 |
| `AliasDefinition` | 下游模型名和有序 deployment candidates | 候选不是完整的协议路径 |

当前 `ProviderKind` 是闭合集合，包含 `OpenAi` 与 `Meituan`。`providers::compiled_definition()` 显式注册 OpenAI 与 Meituan/LongCat 的 provider、model、deployment 和 public alias；不存在动态插件发现。

### 3.2 启动编译

`build_registry` 按 Model → Provider/Credential → Deployment → Alias 的顺序验证并构建 `RegistrySnapshot`。主要约束包括：

- ID 唯一且引用完整；
- credential locator 与 adapter credential kind 合法；
- endpoint profile、HTTPS base URL 和 timeout 合法；
- deployment capability 不能超过 `ProviderDescriptor` 上界；
- deployment model constraint 只能收窄模型事实；
- alias 至少包含一个存在且不重复的 deployment candidate。

成功后生成只读映射：

```text
RegistrySnapshot
  models: id → ModelMetadata
  providers: id → ResolvedProvider
  deployments: id → ResolvedDeployment
  aliases: public name → ResolvedAlias
```

当前没有 reload 或 snapshot 原子替换；整个进程生命周期使用同一份 snapshot。

## 4. HTTP 接入层

实现位置：

- `src/ingress/mod.rs`
- `src/ingress/auth.rs`

公开接口：

| Endpoint | 当前处理 |
|---|---|
| `GET /healthz` | 返回状态与注册表版本 |
| `GET /v1/models` | 枚举 `RegistrySnapshot.public_aliases()` |
| `POST /v1/chat/completions` | 进入 Chat Native Path |
| `POST /v1/responses` | 进入 Responses Native Path |

接入层负责：

- request body limit、request id、trace 和敏感认证头标记；
- 对 `/v1/*` 施加静态 Bearer 认证；
- 验证唯一且合法的 `application/json` content type；
- 把入口协议、原始 JSON bytes 和共享 `AppState` 交给 native forwarding；
- 将本地路由错误转换为稳定的 OpenAI-compatible JSON error。

`AppState` 只持有共享服务句柄，不包含每请求可变路由状态。

## 5. 请求分析与路由层

实现位置：

- `src/core/request.rs`
- `src/core/capability.rs`
- `src/pipeline/mod.rs`

`prepare_native_request` 从请求中提取：

- public `model` 和入口协议；
- streaming；
- function/custom/未建模 tool；
- parallel tool calls；
- image input、structured output、store；
- Responses `previous_response_id`、background；
- reasoning 与 reasoning level；
- 可解析的最大输出限制。

当前路由过程：

```text
public alias
→ ordered deployment candidates
→ protocol-specific CapabilitySet gate
→ model limits and reasoning gate
→ PreparedNativeRequest
```

`PreparedNativeRequest` 保存有序 `PreparedNativeCandidate`，每个候选只固定 `deployment_id + ValidatedRequest`。请求 body 仍保持原始 bytes；pipeline 不改写上游 model，也不执行协议转换。

若请求携带 `previous_response_id`，pipeline 禁止跨 deployment fallback。当前没有通用 continuation ledger、route-local conversion policy、完整 ServingRoute 或独立 RoutePlan 类型。

## 6. Provider 适配层

实现位置：

- `src/provider/*`
- `src/providers/openai.rs`
- `src/providers/meituan.rs`

公共层提供闭合 `ProviderAdapter` dispatch 以及请求、认证/header、响应/SSE terminal 和错误分类契约。具体 Provider 模块负责：

- endpoint profile 与相对 path；
- 从环境 credential lease 构造敏感认证 header；
- 把 public model 改写为 deployment 的 `upstream_model`；
- 识别 Chat `[DONE]` 与 Responses terminal event；
- 分类安全错误与首输出前 retry hint。

Ingress 和 transport 不根据 provider 名称拼接认证规则。新增 Provider 必须新增 `ProviderKind` 变体、descriptor、adapter、注册项和测试。

## 7. 上游 Transport 与 SSE 层

实现位置：

- `src/transport/upstream.rs`
- `src/transport/sse.rs`

共享 `UpstreamClient` 负责：

- 复用 reqwest 连接池并禁止 redirect；
- 仅将 adapter 生成的相对 URI 与已校验 endpoint base 合并；
- 应用 deployment request timeout；
- 以流式 body 返回上游 status、headers 和 bytes。

对于成功的 streaming response，Ingress 使用 `SseDecoder` 观察 UTF-8、framing、event size 和 terminal，但不重新渲染业务 event。body 被下游丢弃时，上游 byte stream 随之 drop。

当前 retry/fallback 直接编排在 `ingress::forward_native`：仅 streaming 请求会对可重试 status、连接错误或 timeout 做固定次数的首输出前尝试，并在允许时转到下一个 deployment candidate。当前没有独立 AttemptManager、统一总预算或跨请求 cooldown。

## 8. Probe 与验证层

实现位置：

- `src/probe.rs`
- `src/bin/openbridge-probe.rs`
- `tests/*`
- `testdata/*`
- `tools/corpus/*`

`openbridge-probe` 复用同一 bootstrap、注册表、Provider adapter 和 transport，可执行模型发现、最小 Chat/Responses 请求及 function call/result replay。Probe 不修改 snapshot，也不自动把观察结果提升为 capability。

测试通过可注入 `UpstreamTransport` 和 credential source 隔离真实网络与 secret，并覆盖注册表、认证、路由、model rewrite、HTTP/SSE、retry/fallback 和取消边界。独立 corpus/testkit 与网关运行时分离。

## 9. 当前依赖方向

```text
main/config
  → providers/models/registry
  → ingress

ingress
  → pipeline
  → provider contracts
  → transport

pipeline
  → core request/capability
  → registry snapshot

provider dispatch
  → concrete providers
  → core protocol/request

transport
  → resolved deployment
```

当前最明显的结构耦合是：Ingress 同时承担 candidate attempt 编排与 SSE response 观察；`registry/mod.rs` 同时包含定义、校验、resolved 类型和 builder；`DeploymentDefinition` 同时承担共享上游调用边界与两个协议的能力声明。这些是迁移输入，不应在现状文档中描述为已解决。

## 10. 当前未实现边界

- `RealModelDefinition`、`UpstreamTargetDefinition`、`NativeOfferingDefinition`、`PublicModelDefinition` 和 `ServingRouteDefinition`；
- 代码拥有的 converter catalog、route-local `ConversionPolicy` 和 `ResolvedBridgePlan`；
- Chat ↔ Responses Protocol Bridge；
- 基于完整执行路径交集的 capability 编译；
- 上游明确不支持后的异构 route fallback；
- 跨请求 cooldown、动态 availability overlay 和独立 AttemptManager；
- 模型与能力信息私有扩展接口；
- route reload 或运行时 Provider DSL。

## 关联文档

- [当前实现说明](current-implementation.md)
- [目标服务架构](../implementation-plans/service-architecture.md)
- [代码注册表与路由](../implementation-plans/configuration-and-routing.md)
- [注册表与路由架构迁移计划](../implementation-plans/registry-architecture-migration.md)
- [Provider adapter 与数据流](../implementation-plans/provider-adapters-and-dataflow.md)
