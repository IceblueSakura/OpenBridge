# 当前代码架构

## 状态与边界

**已实现事实。** 当前生产注册表使用
`ModelConfig`、`ProviderInstanceConfig`、`CredentialPoolConfig`、`UpstreamTargetConfig`、`UpstreamApiConfig`、
`PublicModelConfig` 与 `RouteConfig`，请求路径使用 `RequestRequirements + RoutePlan`。 Embeddings 使用独立的
`EmbeddingRequestRequirements + EmbeddingRoutePlan`，不进入 generation-only
`ApiProtocol` 或 `BridgePlan`。 本页不复制易漂移的测试数量；各功能页的实际运行命令、结果和未执行验收层统一见
[实施现状目录](README.md)及其专题页。

当前生产请求同时支持 Native Path 与显式 `Bridged` Route。请求级 `AttemptManager`、单进程跨请求 cooldown、`BridgePlan`、双向
JSON/SSE renderer 和 stream 状态机已经接入统一 ingress；模型信息扩展接口 与固定 Public Model 能力预检也已接入。Embeddings
另有严格 JSON ingress、单条 Native Route 和预提交有界成功体校验。请求生命周期观测已接入 OpenTelemetry traces/metrics；
显式配置时通过 bootstrap-owned OTLP/HTTP collector 导出脱敏 request/attempt spans，以及由 SDK Counter/Histogram 聚合的
request/attempt、韧性、timing、usage 和 cache metrics。旧进程内快照与 JSON metrics endpoint 已删除；OTLP logs 与内置
Prometheus exporter 尚未接入。

## 1. 分层结构

```text
bootstrap / users / upstream credentials
          ↓
composition root
          ↓
immutable RuntimeRegistry + UserRegistry + CredentialStore + OAuth2CredentialManager
          ↓
HTTP Models projection / request ingress
          ↓
operation-specific requirements → Public Model interface preflight → RoutePlan / optional BridgePlan
          ↓
ProviderAdapter + ProviderInstance + UpstreamTarget + UpstreamApi
          ↓
shared UpstreamTransport / SSE observation or bridge rendering
          ↓
upstream provider

response body EOF / error / drop
          ↓
tracing lifecycle + OpenTelemetry Counter/Histogram instruments
```

依赖方向保持单向：配置和注册表不执行网络 I/O；pipeline 不按 Provider 名称分支；adapter 不选择 Public Model 或
Route；transport 不解释模型和协议能力。

### 1.1 关键代码词汇

| 层            | 核心类型                                                                                                                                                                                                               | 简单定义                                                                                            |
|---------------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|-----------------------------------------------------------------------------------------------------|
| 启动配置      | `BootstrapConfig`、`RuntimeLimits`、`HttpClientConfig`                                                                                                                                                                 | 进程启动参数、request/response/replay/SSE 限制和 HTTP client 参数                                   |
| Credential    | `CredentialPoolConfig`、`CredentialPoolBinding`、`CredentialId`、`CredentialStoreBuilder`、`CredentialStore`、`OAuth2CredentialManager`、`UpstreamCredential`                                                          | 启动时解析 binding，分别冻结 API key 与 OAuth2 bundle，并提供受限只读视图                            |
| 下游身份      | `UserConfigPath`、`UserConfiguration`、`UserRegistry`、`User`                                                                                                                                                          | 启动时分离用户元数据与 Key，通过 Store 匹配后提供稳定用户身份                                       |
| 上游凭证      | `UpstreamCredentialConfigPath`、`UpstreamCredentialConfiguration`                                                                                                                                                      | 校验私有 TOML，并按编译期 binding id 分别装载有序 API key 或单一 OAuth2 auth 文件                    |
| API 语义      | `OperationKind`、`ApiProtocol`、`ApiRequest`、`EmbeddingRequest`、`ApiCapabilities`、`ChatCompletionsCapabilities`、`ResponsesCapabilities`、`EmbeddingsCapabilities`、`GenerationCapabilities`                        | 独立 operation、generation 协议/请求、Embeddings 请求、分域能力和仅供内部判定使用的公共生成能力投影 |
| 注册配置      | `ModelConfig`、`ProviderInstanceConfig`、`UpstreamTargetConfig`、`UpstreamApiConfig`、`RouteConfig`、`PublicModelConfig`                                                                                             | 编译期写入并等待校验的配置                                                                          |
| 运行注册表    | `RuntimeRegistry`、`ModelInfo`、`ProviderInstance`、`PublicModelInfo`、`StandardModel`、`ModelInterfaceCapabilities`、`EmbeddingInterfaceCapabilities`、`ModelExecutionInterface`、`UpstreamTarget`、`UpstreamApi`、`Route`、`PublicModel` | 校验通过后供模型接口和请求路径共同只读使用的数据                                                    |
| 请求规划      | `RequestRequirements`、`RoutePlan`、`RouteCandidate`、`EmbeddingRequestRequirements`、`EmbeddingRoutePlan`                                                                                                             | 请求需要什么、可走哪些固定 route、每条 route 绑定到哪里                                             |
| Bridge        | `BridgePlan`、`BridgeStreamRenderer`、`ChatStreamState`、`ResponsesStreamState`                                                                                                                                        | 受限双向请求/响应转换及单请求 stream lifecycle、tool identity 与 arguments 重建                     |
| Provider      | `ProviderContract`、`ProviderAdapter`、`PreparedUpstreamRequest`                                                                                                                                                       | Provider 能力上界、闭合实现分派和待发送请求                                                         |
| Transport     | `UpstreamTransport`、`UpstreamClient`、`UpstreamResponse`                                                                                                                                                              | 可替换的发送边界、生产 HTTP client 和上游响应                                                       |
| Observability | `RequestObservation`、`FirstOutputCapture`、`ProviderAttemptObservation`、`TelemetryRuntime`、`GatewayMetrics` | 请求/attempt trace 生命周期、原始 upstream body/SSE 观测、SDK metrics instruments 与 OTLP export |
| Probe         | `ProbeOptions`、`ProbeResult`、`TargetProbeReport`                                                                                                                                                                     | 探测输入、单项观察和 target 汇总报告                                                                |

命名规则保持简单：`*Config` 表示构建前配置，去掉 `Config` 表示校验后的运行实体，`*Info` 表示只读事实，
`*Result` 表示一次操作结果，`*Error` 表示失败。`UpstreamTransport` 是当前唯一公开 trait，只用于隔离真实 HTTP 发送与可控
transport；Provider 的请求、认证、SSE 和错误处理统一由闭合 `ProviderAdapter` 直接分派， 不再拆成多组单方法 trait。

### 1.2 源码模块所有权

当前模块按独立责任和协议域拆分，不以文件行数作为边界。单一状态机、Store、transport 或编译规则即使较大仍保持内聚；只有同时拥有
不同变化原因的文件才下沉私有子模块。`mod.rs`、`*.rs` facade 与同名目录只是 Rust 物理组织，不构成下游兼容契约。

| 责任域                    | 当前模块                                                                                                  | 边界                                                                                         |
|---------------------------|-----------------------------------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------|
| 进程装配与启动输入        | `main.rs`、`config/*`、`identity.rs`、`upstream_credentials.rs`、`oauth2_credentials.rs`                  | 启动时读取、校验并冻结配置；不进入请求期动态注册                                             |
| Credential 存储          | `credential.rs`                                                                                           | 统一 purpose-bound secret Store 与受限借用；不拥有文件格式或 Route                           |
| 协议与能力事实            | `core/request.rs`、`core/capability.rs`、`core/capability/{generation,embeddings}.rs`                     | facade 只汇总公共类型；generation 与 Embeddings 的字段、校验和 subset 规则分域               |
| Canonical Model/Provider  | `models/*`、`provider/*`、`providers/*`                                                                   | Model 事实、Provider 抽象与受信具体实现分开；显式 catalog 负责装配                            |
| Registry                  | `registry/{definition,runtime,validation,compiler}.rs`、`registry/public_model*`                          | 配置、校验、运行实体、公共 DTO、私有执行快照和编译算法不互相混放                             |
| 请求分析与规划            | `pipeline/analysis*`、`pipeline/{types,preflight,planning,error}.rs`                                      | analysis 只提取请求事实；preflight 只读固定契约；planning 只消费同一执行接口的固定候选        |
| HTTP 与上游传输           | `ingress/*`、`transport/*`                                                                                | ingress 拥有认证、响应与 attempt 生命周期；transport 只发送受信相对请求并处理 HTTP/SSE framing |
| Protocol Bridge          | `bridge.rs`、`bridge/*`                                                                                   | facade、请求/响应转换与双协议 stream 状态机分开；不选择 Provider 或 Route                     |
| 观测与管理员探测          | `observability.rs`、`observability/*`、`probe.rs`、`probe/*`                                              | 观测不保存业务正文或 secret；probe 只使用已注册且显式选择的 target                            |

本轮结构审计将三个混合职责的根文件收敛为 facade：`core/capability.rs` 按 generation/Embeddings 分域，
`pipeline/analysis.rs` 按 generation/Embeddings 请求分析分域，`registry/public_model.rs` 将下游 DTO、私有执行快照、契约聚合与
Embeddings response budget 编译分开。原有 `openbridge::core::*`、`openbridge::pipeline::*`、`openbridge::registry::*` 重导出路径和
运行数据流保持不变。

## 2. 装配与配置层

实现位置：`src/main.rs`、`src/config/*`、`src/providers/catalog.rs` 与
`src/providers/catalog/*`；`src/config/mod.rs` 保留基础配置定义与重导出，
`src/providers/mod.rs` 只保留包入口。

启动顺序：

```text
BootstrapConfigPath::load
→ TelemetryRuntime::from_bootstrap + init_tracing
→ UserConfigPath::load
→ UpstreamCredentialConfigPath::load
→ derive active pool IDs
→ providers::build_compiled_registry_with_active_pools
→ UpstreamCredentialConfiguration::load_into_for
→ CredentialStore::validate_registry
→ immutable CredentialStore + OAuth2CredentialManager
→ UpstreamClient::new
→ GatewayState::new_with_oauth2_credentials(...).with_metrics(...)
→ OAuth2CredentialManager::run_refresh_scheduler
→ ingress::build_router
→ axum::serve
→ TelemetryRuntime::shutdown
```

`bootstrap.toml` 拥有 loopback listener、两份私有 credential 文件位置、request/JSON response/replay/SSE 大小和 HTTP client
参数，以及分别默认禁用的 `[telemetry.traces]`、`[telemetry.metrics]` OTLP/HTTP base URL。四个 limit 都是必填非零值，replay limit 不得超过 request limit；
exporter 接受带有效 loopback、非 loopback IP 或 DNS host 的绝对 `http` URL，固定 `/v1/traces`、`/v1/metrics` 和代码内
processor/reader/timeout/shutdown 策略，不接受 URL credential、自定义 path/query/fragment、header 或请求级覆盖；HTTP client 会剥离
环境注入的非协议 header。用户文件、 上游 credential
文件、Provider family、Provider instance、模型、target、upstream API 和 route 都只在启动阶段加载；没有 route TOML、 动态 Provider DSL 或热重载。
`UserConfiguration` 把用户元数据交给 `UserRegistry`、把 Key 交给
`CredentialStoreBuilder`；`UpstreamCredentialConfiguration` 把每个编译期 binding 校验为 `api_keys`、`auth_json_file` 或未激活 source。
解析出的 active pool 集合先参与 Target/Public Model 编译，只有激活 target 引用的 API-key pool 才进入不可变 `CredentialStore`；显式配置的 OAuth2 文件在监听前完成首次读取：缺失文件在
advisory lock 内创建为空并保持待登录，存在且非空的文件完成完整 bundle 校验后进入独立 lifecycle manager，相对 locator 以 upstream TOML 目录为基准。
未知或重复 binding、source/kind 错配、同 Provider 多 auth 文件、损坏 TOML、空白/重复 API key 或非空损坏/不完整 OAuth2 bundle 会在
listener 绑定前失败；缺失 pool、source-less pool 或空 API-key 数组只禁用其引用 Target；完整但过期的 bundle 保留为立即 refresh 输入。manager 对外只发布脱敏 snapshot，对内以 per-credential async gate、同主机 advisory file lock、guarded reload、atomic
replace 和 generation 维护 rotation；worker 按 expiry safety window 调度，并随 HTTP 服务结束而取消。两份 TOML、用户与 API-key Store
仍不热重载，OAuth auth 文件只在明确 login 或 guarded refresh transaction 中写入/读取。

## 3. 注册表层

实现位置：`src/registry/definition.rs`、`src/registry/runtime.rs`、`src/registry/public_model.rs`、
`src/registry/public_model/execution.rs`、`src/registry/public_model/compiler.rs`、
`src/registry/public_model/compiler/{contract,embedding_budget}.rs`、`src/registry/compiler.rs`、
`src/registry/validation.rs`、`src/models/*`、`src/providers/*`；`src/registry/mod.rs`
只保留包入口与公共重导出。

`public_model.rs` 只拥有下游安全 DTO 与 preflight accessor；`execution.rs` 保存不序列化的 operation interface/candidate；
`compiler.rs` 只编排静态候选与投影构建，`contract.rs` 负责每 Route 贡献和保守交集，`embedding_budget.rs` 负责 checked
worst-case JSON budget。Registry 总编译器仍通过 facade 调用这一边界，不依赖私有子模块路径。

```text
RegistryConfig
  models: ModelConfig[]
  provider_instances: ProviderInstanceConfig[]
  credential_pools: CredentialPoolConfig[]
  upstream_targets: UpstreamTargetConfig[]
    upstream_apis: UpstreamApiConfig[]
  routes: RouteConfig[]
  public_models: PublicModelConfig[]
```

各实体职责：

| 实体                   | 所有内容                                                                                                             |
|------------------------|----------------------------------------------------------------------------------------------------------------------|
| `ProviderContract`     | Provider adapter 代码拥有的 credential kind 与能力上界                                                               |
| `ProviderInstanceConfig` | 一个稳定实例 ID、一个 `ProviderKind` 与唯一受信 BaseURL；不同 URL/区域使用不同实例                                  |
| `ModelConfig`          | 显式 canonical profile 的模型事实、total/input/output context、mode、模态、参数与 reasoning 元数据；不同 profile 的已核实事实可分开注册 |
| `CredentialPoolConfig` | 非敏感 pool id、Provider 与 credential kind                                                                          |
| `UpstreamTargetConfig` | Provider instance、Model、credential pool 引用、timeout、启停及 quota/fault 边界                                     |
| `UpstreamApiConfig`    | 单一原生 operation 的 upstream model、served limits、能力证据、state affinity 与可选 reasoning level 映射            |
| `RouteConfig`          | target、typed upstream operation、下游 operation 和 `Native`/`Bridged` 执行模式                                      |
| `PublicModelConfig`    | 下游稳定 id、创建时间、展示元数据、生命周期与私有有序 Route ID                                                       |
| `PublicModelInfo`      | 标准身份、模型事实及每 operation 唯一固定能力契约；不包含任何部署字段                                                |

当前编译目录包含 20 个 `ModelConfig`：19 个 generation 模型，以及独立的
`openai/text-embedding-3-small` Embedding 模型。通常同一研发者命名空间由 `src/models/<developer>.rs` 聚合；ChatGPT subscription
侧因已核实 context profile 不同，使用独立的 `src/models/chatgpt.rs` namespace，当前包含 Spark、GPT-5.5 以及 GPT-5.6
Luna/Terra/Sol 共 5 个 profile。目录下每个扁平叶模块只定义一个具体模型。版本、
checkpoint 和命名变体直接组成 snake_case 模块名：例如 `openai/gpt_5_6_sol.rs`、`chatgpt/gpt_5_3_codex_spark.rs`、
`deepseek/deepseek_v4_flash.rs`、`xiaomi/mimo_v2_5_pro.rs` 与 `qwen/qwen3_7_max.rs`；不增加版本聚合层。各根模块直接维持
目录顺序，源码使用 `openai::gpt_5_6_sol::ID` 或 `chatgpt::gpt_5_3_codex_spark::ID` 这类扁平作用域名称。OpenRouter 的 `z-ai` slug 在 Rust 路径中使用
`z_ai`，其他点号与连字符同样规范化为下划线。每个具体模型仍完整拥有 id、名称、context、 参数、reasoning 状态和
level，不从共享默认值拼装模型字段；mode 与模态可作为显式已知事实进入扩展信息。目录存在不等于 可调用；只有被 Upstream Target
引用并进入 Public Model Route 的模型才会参与规划或出现在 `/v1/models`。服务启动还会根据私有凭证配置派生 active pool 集合；缺失、无 source
或空 API-key pool 会使引用它的 Target/Public Model 在本次运行中不可执行，但不会删除代码注册的 Provider 或 canonical Model。
`ModelConfig` 已分型表示 Chat 与 Embedding，但仍没有 rerank task；两个 Nemotron retrieval 条目没有因此被 伪装成可调用
Embedding/rerank 模型。其中 OpenRouter 精确匹配的模型已补齐现有字段；
`chatgpt/gpt-5.3-codex-spark` 没有精确目录项，其 context、输出和 level 是人工修订值。外部事实与 Nemotron
`:free` 变体边界见 [OpenRouter 模型目录快照](../references/providers/openrouter/model-catalog-2026-08-02.md)。
ChatGPT GPT-5.5/5.6 profiles 复制对应 OpenAI model facts，但 canonical context/input limits 独立收窄为 272,000，最大输出保持
128,000。Spark 与 GPT-5.6 Luna/Terra/Sol 已分别进入固定 target、Responses-native Route 和 Public Model；GPT-5.5 仍只有目录 profile。

同一 generation target 可以同时注册 Chat 和 Responses Upstream API；二者可拥有不同 upstream model、 context/output
限制、能力证据和 state affinity。API operation 只由 capabilities variant 决定，同一 Target 对每个 `OperationKind` 最多一份；
Route 和执行候选以 typed operation 引用 API。Embeddings checked-in 注册使用独立 target，只包含一个 Embeddings API。
BaseURL 只属于 Provider instance；credential、Model、timeout 与故障边界仍属于 target。

`build_registry` 先验证 Provider instance ID 与唯一 HTTPS BaseURL，再验证 Target 引用、credential、timeout、Provider 上界、
Upstream API operation 唯一性与能力、model
rules 只收窄、三段 context 关系、Native/Bridged route 方向、Embeddings 单 Native candidate 与闭合 capability、Public Model
身份/生命周期及 route 顺序。公共对象与请求预检必须保持的需求见
[Public Model 与模型能力契约](../functional-requirements/model-information-and-capability-contract.md)。随后按 operation
对 所有静态可执行 Route 做保守交集，预编译 `PublicModelInfo`；成功后生成：

```text
RuntimeRegistry
  models
  provider_instances
  credential_pools
  upstream_targets → upstream_apis
  routes
  public_models
```

## 4. HTTP 接入层

实现位置：`src/ingress/*`；其中 `router.rs` 负责服务装配，`handlers.rs` 负责 endpoint，
`forwarding.rs` 负责 generation candidate/retry/fallback，`forwarding/embeddings.rs` 负责单 Route Embeddings attempt，
`forwarding/embeddings/response.rs` 负责有界成功体校验，`forwarding/response.rs` 负责把 已选 generation 上游响应交给
Native 或 Bridged 返回路径，`streaming.rs` 负责 SSE 生命周期，`response.rs` 与 `lifecycle.rs` 分别负责响应归一化和 请求终态观测。

| Endpoint                            | 当前处理                                                            |
|-------------------------------------|---------------------------------------------------------------------|
| `GET /healthz`                      | 返回状态与注册表版本                                                |
| `GET /v1/models`                    | 返回 Public Model 的 OpenAI 标准四字段列表                          |
| `GET /v1/models/{model}`            | 返回一个标准四字段 Model 对象                                       |
| `GET /openbridge/v1/models`         | 返回完整 Public Model 能力列表                                      |
| `GET /openbridge/v1/models/{model}` | 返回一个完整 Public Model 能力对象                                  |
| `POST /v1/chat/completions`         | 进入 Chat Native/Bridged RoutePlan                                  |
| `POST /v1/responses`                | 进入 Responses Native/Bridged RoutePlan                             |
| `POST /v1/embeddings`               | 进入严格 JSON Embeddings analysis/preflight 与唯一 Native candidate |

Ingress 执行认证、body/content-type 限制、本地错误归一化和当前的首输出前 attempt 循环。它不接受 客户端提供的上游
URL、credential 或内部 route ID。

## 5. 请求分析与路由层

实现位置：`src/core/request.rs`、`src/core/capability.rs`、`src/core/capability/{generation,embeddings}.rs`、
`src/pipeline/types.rs`、`src/pipeline/analysis.rs`、`src/pipeline/analysis/{generation,embeddings}.rs`、
`src/pipeline/preflight.rs`、`src/pipeline/planning.rs`；`src/pipeline/mod.rs` 只保留包入口与公共重导出。

`core/capability.rs` 只在 `ApiCapabilities` 汇总 operation family；generation 与 Embeddings 子模块分别拥有自己的闭合字段、校验和
subset 规则。`pipeline/analysis.rs` 只重导出两个 analyzer：generation analyzer 处理 Chat/Responses 请求事实，Embeddings analyzer
处理严格 input union 与 endpoint 字段；二者都不查询 registry、不构造 RoutePlan，也不改写 body。

```text
raw body + downstream operation
→ analyze_request / analyze_embedding_request
→ RequestRequirements / EmbeddingRequestRequirements
→ Public Model operation execution interface（fixed capability + static candidates）
→ operation-specific capability and limit preflight
→ RoutePlan<RouteCandidate> / EmbeddingRoutePlan
```

`RequestRequirements` 只记录 generation 请求事实：public model、协议、streaming、功能组合、输出限制和状态亲和指示。 reasoning
level parser 识别 `none`、`minimal`、`low`、`medium`、`high`、`xhigh` 与 `max`；`none` 保持为 显式 level，字段缺失才表示调用方没有请求
reasoning。
`EmbeddingRequestRequirements` 只保存 input form/count、可本地计算的 token counts、可选 encoding/dimensions 和 `user`
是否出现，不复制业务输入。 registry compiler 在完成 Route、Target 与 Upstream API 引用和方向校验后，为每个 Public Model/下游
operation 编译一个
`ModelExecutionInterface`。它把 generation 或 Embeddings capability 与同一组静态启用候选保存在一起；候选冻结 Route、
Target、Upstream API、上下游协议和 `Native`/`Bridged` 模式。`PublicModelInfo` 只投影这份固定能力契约的安全副本， 不包含候选拓扑。
`preflight` 从该执行接口读取能力，因此不支持或未知能力在查看候选前失败；通过后 `planning` 只遍历同一接口的 固定候选，不再扫描
`PublicModel` 的原始 Route ID，也不重复检查 Target/API 静态启停或协议资格，并在需要时生成
`BridgePlan`。Bridge 构造失败仍拒绝整个请求，不能跳过该候选。 Native candidate 保留 canonical `ApiRequest`，Bridged
candidate 保存目标协议的 canonical `ApiRequest`；两者都不在 RoutePlan 中应用或记录 reasoning wire 映射。 Embeddings
preflight 读取同一执行接口的四种 input form、encoding/dimension domain 和有效 limit；planning 只接受 唯一 Native
candidate，并把原始 `EmbeddingRequest` 交给 adapter 在 egress 时改写受信 model/path。

Provider adapter 在选定候选进入 egress 准备时一次性解析 JSON，写入真实 model，并按该 Upstream API 的显式配置 把 canonical
reasoning level 改为安全 wire 值。映射源必须属于有效 Model 的 level 集合，目标值必须满足受限 wire 命名规则，同一源不得重复；没有映射的候选保持
canonical level，未知下游 level 仍在 preflight 失败关闭。

请求携带 `previous_response_id` 时，计划关闭跨 target fallback。registry 还要求全部 Responses Route 的 continuation issuer
唯一解析到同一 Target/API；多个潜在签发者会把固定能力收窄为 `unsupported`，并在规划前 拒绝请求。不同 Route 或 Upstream API
的其他能力只在 registry 构建时做保守交集，绝不按字段求并集；请求能力 不用于跳过较弱 Route 选择较强 Route。

## 6. Provider 适配层

实现位置：`src/provider/kind.rs`、`src/provider/definition.rs`、`src/provider/adapter.rs`、`src/provider/contracts.rs`、
`src/providers/openai_compatible.rs`、各 `src/providers/<provider>.rs` 根模块，以及同名目录中的
`definition.rs` 与可选 `registration.rs`；`src/provider/mod.rs` 和 `src/providers/mod.rs` 只保留包入口， 具体 Provider 不使用
`mod.rs`。

`ProviderKind` 是闭合集合。每个具体 Provider 以一个静态 `ProviderDefinition` 聚合自己的 contract 与 adapter；
`ProviderKind::definition` 是 kind 到具体 definition 的唯一穷举分派，`ProviderKind::contract` 与
`ProviderAdapter::for_kind` 都委托给它。OpenAI、LongCat、OpenRouter、DeepSeek、MiMo、ChatGPT、NVIDIA 与百炼的独立静态定义拥有
Provider 契约、endpoint path、`ProviderRequestHeaders`、request header/body hook 与 Responses terminal discriminator；共享
`openai_compatible` 机制负责模型字段与 reasoning level wire 映射、认证 header、响应/SSE terminal、错误分类和 generation Upstream API
pair 构造；OpenAI adapter 另注册固定 `/v1/embeddings` path。NVIDIA 与百炼只声明基础 `/chat/completions` adapter 和 API-key
credential kind；NVIDIA 绑定一个 MiniMax M3 target，百炼绑定 GLM-5.2 与两个 Qwen3.7 target，四者只生成 Chat Native Route。
DeepSeek adapter 声明 `/chat/completions` 与 `/responses`，但只有 V4 Flash
target 注册 Responses Upstream API；OpenRouter 与 MiMo 同样声明 Chat/Responses 两个 path。Provider hook 可增添、替换、 转换或删除普通
header；`ProviderRequestHeaders` 通过 `StaticRequestHeader` slice 声明固定的非敏感 UA/header，并在 hook 后覆盖同名值；OpenAI 与
LongCat hook 转发 `User-Agent`，OpenRouter hook 不转发可选 attribution/routing header，共享层不维护普通 header allowlist。
`SafeHeaders` 对两条普通 header 路径统一拒绝 Authorization、cookie、Host 与 proxy authorization，credential header 最后独立附加。
credential pool id、Provider 与 credential kind 来自
`CredentialPoolBinding`，endpoint/timeout 来自 selected Upstream Target。Ingress 按完整
`pool_id + member_id + ProviderKind + CredentialKind` 从 `CredentialStore` 借用 `UpstreamCredential`；每个 Store 条目 冻结
credential type、来源类别、generation 与可选过期时间，来源类别不保存文件路径。Store 不公开通用明文查询，adapter 仍在 crate
内的认证 header 边界才访问 secret。五个数据面 Provider 只允许 `ApiKey`；ChatGPT contract 只允许
`OAuth2BearerAccessToken`，其 Provider authentication adapter 要求 access token、account ID、FedRAMP routing flag 与已知 expiry
保持为不可拆分的 credential material，并把 Bearer、account 与条件性 FedRAMP header 全部放入敏感 header 集。API-key TOML 不能填充
OAuth pool；OpenBridge-owned bundle 由独立 `OAuth2CredentialManager` 持有。ChatGPT ingress 从 manager 取得短生命周期、账户绑定 lease，
在 adapter 认证边界消费；首个预提交 `401` 触发 guarded reload、必要时 refresh 和一次重放，第二个 `401` 只终态化仍被拒绝的 generation。

每个 Chat/Responses capability 还声明 `ReasoningOutput`：`Unknown` 不表示可读输出，`PlainText` 和 `Summary`
才允许进入方向兼容的 Bridge reasoning channel，`Opaque`（包括 `encrypted_content`）不会被转换。OpenAI、LongCat 与 MiMo
当前都通过共享构造器注册 Chat、Responses 两个独立 Upstream API；DeepSeek V4 Flash 显式注册同样两个无状态 API，而 V4 Pro 只注册
Chat。OpenAI 另有
`openai-text-embedding-3-small` target、`embeddings` API 和 `text-embedding-3-small-openai-embeddings` Native Route， 不复用
`openai-main` 做请求期模型分支。目录中的每个 generation Public Model 由一个编译注册单元持有有序 Provider route
source；每个下游协议先按 source 顺序生成全部 Native route，再按 相同顺序生成指向相反 Upstream API 的 Bridged route。当前
`deepseek-v4-flash` 显式绑定 DeepSeek 与 OpenRouter 两个 source， 其余 checked-in generation 注册项各只有一个 source。MiMo
的两个 target 分别绑定 `mimo-v2.5-pro` 与 `mimo-v2.5`，共享 `mimo-primary` pool、 quota scope 与 fault domain；前者保留文本
Bridge，后者为支持图片契约而只注册两个同协议 Native Route。Bridge
生产路径由编译注册表、记录型 transport 与 canonical wire 确定性验证， 但尚未调用真实异构协议 Provider。

ChatGPT registration 为 Spark 与 GPT-5.6 Luna/Terra/Sol 固定四个 target、同一个 Codex backend、`responses` path、各自的 upstream
model 和共享 `chatgpt-codex` OAuth pool；四个 Public Model 各有且仅有一个 Responses Native Route。ChatGPT definition 固定
`Accept: text/event-stream`、`originator: codex_cli_rs` 与 `codex_cli_rs/0.146.0 (Linux unknown; x86_64) unknown` UA，要求
`stream: true`，把字符串 `input` 转为 user message 数组并强制 `store: false`，在 egress 前拒绝三个输出 token limit 字段。该 profile 不读取本机 Codex auth、
部署主机 OS/environment/terminal identity，也不调用 Codex executable/app-server；Models probe 使用的
`/models?client_version=0.146.0` query 是编译期固定的 adapter 事实，不由本机 client profile 提供。

静态协议能力现在使用 `ChatCompletionsCapabilities` 与 `ResponsesCapabilities` 分域表达； crate-private
`GenerationCapabilities` 只是公共子集判断使用的投影，不再充当可注册或公共导出的模糊 endpoint 类型。Chat/Responses
`image_input` 使用 source、MIME、detail 和本地可计数 limit 组成的 typed profile；request analyzer 将实际图片事实冻结后交给固定
interface preflight，不以图片能力筛选候选。
`EmbeddingsCapabilities` 独立拥有 input forms、encoding/dimension domain、request limits 与 可选参数，不参与 generation
intersection 或 Bridge。canonical
`ModelConfig` 记录已核实的 `mode`、`input_modalities`、`output_modalities`、tokenizer 和 knowledge cutoff；当前 OpenRouter
精确匹配的 canonical 模型还记录模型级 `context_length` 作为总上下文和输入上限，并记录可用的 最大输出上限。没有精确目录记录的
Codex Spark 继续保留未知字段。Chat 预留 audio/file/custom tool、audio output、 predicted output、web search、prompt
caching、moderation、logprobs 和 multiple choices；Responses 另以
`HostedToolKind`、`ResponseInclude` 及状态字段预留 hosted tool、附加输出、conversation、prompt template 和 context
management。Provider/API definition 仍保持这些未实现的 endpoint 字段为 `None`、`false` 或空集合；进入 registry 编译的 Model
或 Upstream API definition 一旦启用任一预留字段，就会在监听前触发 `unimplemented!`。 请求分析按 Chat/Responses 分域识别相同预留
wire 语义，在 route/egress 前返回 `UnimplementedCapabilities`，由 ingress 映射为稳定的 `unimplemented_request` HTTP
400；未知且尚未进入预留枚举的 tool type 仍走普通 unsupported gate。 因此这些类型位置与请求错误边界都不构成已实现能力声明。

OpenRouter 当前注册固定 target `openrouter-deepseek-v4-flash`、Chat/Responses Upstream API，并作为
`deepseek-v4-flash` Public Model 的第二个 Provider source；两个协议各有一个 OpenRouter Native route，使用基础 upstream model
`deepseek/deepseek-v4-flash`。Responses API 的 state affinity 是 `Unbound`，`store`、
`previous_response_id` 与 `background` 在 capability gate 关闭。未注册 Bridged route 或 `:free` 变体；同协议规划中，这些 route 排在
DeepSeek Native candidate 之后。

DeepSeek 的两个 target 分别绑定 `deepseek-v4-pro` 与 `deepseek-v4-flash`，共享 `deepseek-primary` pool、 quota scope 与
fault domain。`deepseek-v4-pro` target 与 Public Model 仅保留 Chat Native；`deepseek-v4-flash` target 额外注册 `Unbound`
Responses API，并与 OpenRouter source 聚合为两个协议各自按 DeepSeek、OpenRouter 排序的 Native candidates。DeepSeek Chat 的
reasoning output 为 `PlainText`，Responses reasoning output 暂记为 `Unknown`；MiMo 与 LongCat 的 Chat/Responses reasoning output
同样为 `Unknown`，因此 Native-first route 不受影响，但要求可读 reasoning 的 Bridge candidate 会在 egress 前淘汰。DeepSeek Flash
Responses 的 `store`、`previous_response_id` 与 `background` 仍在公共 capability gate 关闭。

## 7. Transport、SSE、attempt 与 health

实现位置：`src/transport/*` 与 `src/ingress/forwarding.rs`。

共享 `UpstreamClient` 只接收已解析 target 和 adapter 生成的相对 URI，禁止 redirect，并应用 target timeout。Native streaming
response 保持业务 bytes 透明并由 `SseDecoder` 观察 framing/terminal；Bridged stream 则按完整 event 增量渲染目标协议
wire。Embeddings success 在首次下游 commit 前按独立 JSON response budget 完整读取并校验；非法成功体不进入 retry。下游丢弃任一
body 时，上游 stream 随之取消。 OpenAI-compatible adapter 统一使用 OpenAI terminal 词汇，并把 discriminator 来源作为编译期
Provider 事实：OpenAI/MiMo 从 SSE `event:` 读取，LongCat/OpenRouter 从 data JSON 顶层 `type` 读取。discriminator 不进入
TOML 或运行时探测；双来源 terminal 冲突时失败关闭，也不把尾随 `[DONE]` 代替 Responses 语义终态。

`ingress::attempt::AttemptManager` 管理单请求 attempt 生命周期：stream/non-stream 与 Embeddings 共享最多 6 次的硬预算，
每候选最多 2 次，attempt 间从 50 ms 起按二倍增长并 capped 到 500 ms；在预算可容纳时为未尝试候选保留 机会。generation
RoutePlan 允许时可进入同一 Public Model 的下一完整候选；Embeddings 当前只有一个 candidate， 且只有请求不超过 replay budget
才能使用第二次本地 attempt。下游取消会销毁 pending send、timer 或 response body，提交 response 后不得再拼接另一上游响应。

Ingress 在 response 建立前用 lifecycle guard 捕获 pending send/backoff 取消，建立后把责任移交给外层
`RequestBodyObserver`；后者直接保留 HTTP data/trailer frame，仅在自身提交真实 EOF 或 body error 后报告 end-stream，并在真实
EOF、body error 或 drop 时提交唯一请求终态。response headers ready、首 body 字节与 SSE 首个 text/tool/reasoning 增量分别计时，避免把
headers ready 误当成 streaming TTFT。成功的非流式 Chat/Responses 以第一个非空下游 JSON body chunk 作为可直接观测的 gateway
响应时刻，但不据此生成 upstream TTFT、generation duration 或 output speed。首输出使用一次性原子门控，下游 SSE 只解析到首个生成
delta；JSON 与 SSE usage 由原始 upstream observer 解析，Embeddings usage 只在成功体通过 endpoint validator 后提交，不再为下游 JSON 重复分配 usage
cache。业务正文不会写入 tracing 或 metrics。Provider attempt 的 operation/route/target/Provider 等受信编译期维度进入 SDK instruments，request/user/
credential/endpoint URL 仍不进入 metric attributes；`GatewayMetrics` 只持有 OpenTelemetry instrument handles，不维护自定义聚合状态。

`ingress::credential_health::CredentialHealth` 与 `ingress::health::TargetHealth` 在所有 `GatewayState` clone 间共享。
前者维护每 pool round-robin cursor，以及按 `member_id + generation` 隔离的 429 cooldown；`Retry-After`
缺失/非法时为 1 秒并封顶 30 秒。后者只把暂时性 5xx/transport failure 隔离到注册表提供的 `fault_domain`。 无状态请求跳过冷却
member/target；启用 continuation 的 target-bound API 在启动时要求单成员 pool，并保持原
target。两类状态都不持久化、不跨进程，也不执行动态权重或后台探测。

`src/bridge.rs` 作为生产 Protocol Bridge 门面；`bridge/chat.rs` 与 `bridge/responses.rs` 分别维护两种 stream 状态机，
`bridge/conversion/request/*`、`response.rs` 与 `stream/*` 分别承担双向请求、非流式响应与增量 SSE 转换。Responses 侧分别固定
response id、item id、call id 和 output index；Chat 侧只用 tool index 关联同一 stream 的分片，不用它替代 call id。两侧要求唯一
terminal 和闭合 JSON object arguments。`BridgePlan` 只接受 显式 allowlist 内的共同 text/function 与明文 reasoning channel
语义；无法表达的字段、opaque continuation 与私有扩展在 egress 前拒绝。

`src/observability.rs` 与 `src/probe.rs` 同样只保留公开门面：前者将 request lifecycle、Provider observation、usage、SDK instruments
与 startup-owned OTLP lifecycle 拆到同名目录。`downstream_request` root 和每个实际 `provider_attempt` child 使用显式 attribute
allowlist；metrics 使用 SDK 原生 cumulative sum/histogram、固定 60 秒 reader 和 1,024 attribute-set 上限。进程持有 tracer/meter
provider 到 Axum 停止并执行有界 shutdown。后者将固定 payload 和受信 probe session 拆到同名目录。

## 8. Probe 与验证层

`openbridge-probe --target <id>` 针对固定 Upstream Target 工作，并按协议选择对应 Upstream API。它复用 target
endpoint、adapter 与 transport，只为管理员选中的 target 构造一个上游 pool 快照并确定性使用首个 member；它不 加载下游用户
Key、不接受 URL/model/header/credential 覆盖，也不修改 `RuntimeRegistry`。probe 只允许已启用 target；API-key target 加载所选 pool，ChatGPT
target 通过 `OAuth2CredentialManager` 借用所选 auth file 的账户绑定 lease。CLI 没有本机 Agent auth、client identity 或 executable selector，
也不打开未选中的 OAuth2 文件。

测试夹具使用 target/upstream API/route 和 operation-specific requirements/plan API。确定性测试保护注册表、 Provider
边界、路由、HTTP/SSE、Bridge、Embeddings 有界 JSON、retry/fallback、credential rotation/cooldown、取消与观测行为；它们 不自动升级为外部
SDK、独立 Python/curl、目标 Agent、真实 Provider、负载或长期运行证据。最新实际执行结果
只在[实施现状目录](README.md)及对应专题页维护。

## 9. 尚未实现

- 动态 Converter catalog、route-local 可配置 ConversionPolicy 与异构 Provider 实测；
- 动态 availability/weight、持久化或分布式 cooldown；
- OTLP logs、内置 Prometheus exporter、指标持久化、历史查询、重置或分布式指标聚合；
- 可安全投影真实 route/upstream API 信息的内部视图与其他未批准的扩展 HTTP API；
- Responses WebSocket、其他 ChatGPT model/API、function/hosted tool、MCP、完整 Agent loop 和动态 Provider/plugin DSL。
- 多 Embeddings candidate、embedding Bridge、向量转换/缓存/索引/检索和 string tokenizer。

## 关联文档

- [当前实现总览](current-implementation.md)
- [遥测指标](telemetry-metrics.md)
- [配置、凭证与受信边界](../functional-requirements/configuration-and-credentials.md)
- [路由与 Provider 韧性](../functional-requirements/provider-resilience.md)
