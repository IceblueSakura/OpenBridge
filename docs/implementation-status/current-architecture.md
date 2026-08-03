# 当前代码架构

## 状态与边界

**已实现事实。** 当前生产注册表使用
`ModelConfig`、`CredentialPoolConfig`、`UpstreamTargetConfig`、`UpstreamApiConfig`、
`PublicModelConfig` 与 `RouteConfig`，请求路径使用 `RequestRequirements + RoutePlan`。
本页不复制易漂移的测试数量；最近实际运行的命令、结果和未执行验收层统一见
[当前实现说明](current-implementation.md#验证状态)。

当前生产请求同时支持 Native Path 与显式 `Bridged` Route。请求级 `AttemptManager`、单进程跨请求
cooldown、`BridgePlan`、双向 JSON/SSE renderer 和 stream 状态机已经接入统一 ingress；模型信息扩展接口
尚未实现。请求生命周期观测已接入 tracing 和无高基数标签的进程内累计值，但尚未接入 OpenTelemetry/
Prometheus exporter。

## 1. 分层结构

```text
bootstrap / users / upstream credentials
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

response body EOF / error / drop
          ↓
tracing lifecycle events + GatewayMetrics
```

依赖方向保持单向：配置和注册表不执行网络 I/O；pipeline 不按 Provider 名称分支；adapter 不选择
Public Model 或 Route；transport 不解释模型和协议能力。

### 1.1 关键代码词汇

| 层 | 核心类型 | 简单定义 |
|---|---|---|
| 启动配置 | `BootstrapConfig`、`RuntimeLimits`、`HttpClientConfig` | 进程启动参数、请求限制和 HTTP client 参数 |
| Credential | `CredentialPoolConfig`、`CredentialPoolBinding`、`CredentialId`、`CredentialStoreBuilder`、`CredentialStore`、`UpstreamCredential` | 启动时解析 pool、合并上下游 secret、隔离用途并提供只读成员借用视图 |
| 下游身份 | `UserConfigPath`、`UserConfiguration`、`UserRegistry`、`User` | 启动时分离用户元数据与 Key，通过 Store 匹配后提供稳定用户身份 |
| 上游凭证 | `UpstreamCredentialConfigPath`、`UpstreamCredentialConfiguration` | 校验私有 TOML，并按编译期 pool id 把有序 API key 移交给 Store builder |
| API 语义 | `ApiProtocol`、`ApiRequest`、`ApiCapabilities` | 下游协议、原始请求和协议能力 |
| 注册配置 | `ModelConfig`、`UpstreamTargetConfig`、`UpstreamApiConfig`、`RouteConfig`、`PublicModelConfig` | 编译期写入并等待校验的配置 |
| 运行注册表 | `RuntimeRegistry`、`ModelInfo`、`UpstreamTarget`、`UpstreamApi`、`Route`、`PublicModel` | 校验通过后供请求路径只读使用的数据 |
| 请求规划 | `RequestRequirements`、`RoutePlan`、`RouteCandidate` | 请求需要什么、可走哪些 route、每条 route 绑定到哪里 |
| Bridge | `BridgePlan`、`BridgeStreamRenderer`、`ChatStreamState`、`ResponsesStreamState` | 受限双向请求/响应转换及单请求 stream lifecycle、tool identity 与 arguments 重建 |
| Provider | `ProviderContract`、`ProviderAdapter`、`PreparedUpstreamRequest` | Provider 能力上界、闭合实现分派和待发送请求 |
| Transport | `UpstreamTransport`、`UpstreamClient`、`UpstreamResponse` | 可替换的发送边界、生产 HTTP client 和上游响应 |
| Observability | `RequestObservation`、`UsageCapture`、`GatewayMetrics` | 请求终态 tracing、usage 解析和低基数累计值 |
| Probe | `ProbeOptions`、`ProbeResult`、`TargetProbeReport` | 探测输入、单项观察和 target 汇总报告 |

命名规则保持简单：`*Config` 表示构建前配置，去掉 `Config` 表示校验后的运行实体，`*Info` 表示只读事实，
`*Result` 表示一次操作结果，`*Error` 表示失败。`UpstreamTransport` 是当前唯一公开 trait，只用于隔离真实
HTTP 发送与可控 transport；Provider 的请求、认证、SSE 和错误处理统一由闭合 `ProviderAdapter` 直接分派，
不再拆成多组单方法 trait。

## 2. 装配与配置层

实现位置：`src/main.rs`、`src/config/*`、`src/providers/catalog.rs` 与
`src/providers/catalog/*`；`src/config/mod.rs` 保留基础配置定义与重导出，
`src/providers/mod.rs` 只保留包入口。

启动顺序：

```text
BootstrapConfigPath::load
→ UserConfigPath::load
→ UpstreamCredentialConfigPath::load
→ providers::build_compiled_registry
→ UpstreamCredentialConfiguration::load_into_for
→ CredentialStore::validate_registry
→ immutable CredentialStore
→ UpstreamClient::new
→ GatewayState::new
→ ingress::build_router
→ axum::serve
```

`bootstrap.toml` 拥有 loopback listener、两份私有 credential 文件位置、请求/SSE 大小和 HTTP client 参数。用户文件、
上游 credential 文件、Provider、模型、target、upstream API、route 和 endpoint 都只在启动阶段加载；没有 route TOML、
动态 Provider DSL 或热重载。`UserConfiguration` 把用户元数据交给 `UserRegistry`、把 Key 交给
`CredentialStoreBuilder`；`UpstreamCredentialConfiguration` 再为所有启用 target 引用的 pool 解析 TOML `api_keys`
数组。未知、缺失或重复 pool、损坏 TOML、空数组、空白成员或重复 secret 会在 listener 绑定前
失败。构造后的 Store 是上下游 secret 的唯一运行时所有者，请求路径不再读取 credential 来源。

## 3. 注册表层

实现位置：`src/registry/definition.rs`、`src/registry/runtime.rs`、`src/registry/compiler.rs`、
`src/registry/validation.rs`、`src/models/*`、`src/providers/*`；`src/registry/mod.rs`
只保留包入口与公共重导出。

```text
RegistryConfig
  models: ModelConfig[]
  credential_pools: CredentialPoolConfig[]
  upstream_targets: UpstreamTargetConfig[]
    upstream_apis: UpstreamApiConfig[]
  routes: RouteConfig[]
  public_models: PublicModelConfig[]
```

各实体职责：

| 实体 | 所有内容 |
|---|---|
| `ProviderContract` | Provider 代码拥有的 endpoint profile、credential kind 与能力上界 |
| `ModelConfig` | 与供应商无关的模型事实、context、参数与 reasoning 元数据 |
| `CredentialPoolConfig` | 非敏感 pool id、Provider 与 credential kind |
| `UpstreamTargetConfig` | Provider Family、Model、endpoint、credential pool 引用、timeout、启停及 quota/fault 边界 |
| `UpstreamApiConfig` | 单一原生协议的 upstream model、served limits、能力证据、transport、state affinity 与 reasoning level 映射 |
| `RouteConfig` | target、upstream API、下游协议和 `Native`/`Bridged` 执行模式 |
| `PublicModelConfig` | 下游稳定模型名与有序完整 Route ID |

当前编译目录包含 17 个 `ModelConfig`：LongCat-2.0，以及从 LiteLLM 部署清单整理出的 16 个唯一
Chat/Responses 模型。同一模型家族由 `src/models/<family>.rs` 聚合，家族目录下每个扁平叶模块只定义一个
具体模型。版本、checkpoint 和命名变体直接组成模块名：例如 `gpt/v5_6_sol.rs`、
`deepseek/v4_flash.rs`、`mimo/v2_5_pro.rs` 与 `qwen/v3_7_max.rs`；不增加版本聚合层。家族根模块直接维持
目录顺序，源码使用 `gpt::v5_6_sol::ID` 这类扁平作用域名称。每个具体模型仍完整拥有 id、名称、context、
参数、reasoning 状态和 level，不从共享默认值拼装模型字段。目录存在不等于
可调用；只有被 Upstream Target 引用并进入 Public Model Route 的模型才会参与规划或出现在 `/v1/models`。
当前 `ModelConfig` 不表示 embedding/rerank，因此两个 Nemotron retrieval 条目没有被伪装成文本模型。
其中 16 个模型已按 2026-08-02 OpenRouter 官方目录精确匹配并补齐现有字段；
`openai/gpt-5.3-codex-spark` 没有精确目录项，其 context、输出和 level 是人工修订值。外部事实与 Nemotron
`:free` 变体边界见 [OpenRouter 模型目录快照](../references/openrouter/model-catalog-2026-08-02.md)。

同一 target 可以同时注册 Chat 和 Responses Upstream API；二者可拥有不同 upstream model、context/output
限制、能力证据和 state affinity。共享 endpoint、credential、Model 与故障边界属于 target。

`build_registry` 验证引用、唯一性、credential、HTTPS endpoint、timeout、Provider 上界、Upstream API
协议/能力一致性、model rules 只收窄、Native/Bridged route 协议方向及 Public Model route 顺序。成功后生成：

```text
RuntimeRegistry
  models
  credential_pools
  upstream_targets → upstream_apis
  routes
  public_models
```

## 4. HTTP 接入层

实现位置：`src/ingress/*`；其中 `router.rs` 负责服务装配，`handlers.rs` 负责 endpoint，
`forwarding.rs` 负责 candidate/retry/fallback，`forwarding/response.rs` 负责把已选上游响应交给 Native 或
Bridged 返回路径，`streaming.rs` 负责 SSE 生命周期，`response.rs` 与 `lifecycle.rs` 分别负责响应归一化和
请求终态观测。

| Endpoint | 当前处理 |
|---|---|
| `GET /healthz` | 返回状态与注册表版本 |
| `GET /v1/models` | 枚举 Public Model 名称 |
| `POST /v1/chat/completions` | 进入 Chat Native/Bridged RoutePlan |
| `POST /v1/responses` | 进入 Responses Native/Bridged RoutePlan |

Ingress 执行认证、body/content-type 限制、本地错误归一化和当前的首输出前 attempt 循环。它不接受
客户端提供的上游 URL、credential 或内部 route ID。

## 5. 请求分析与路由层

实现位置：`src/core/*`、`src/pipeline/types.rs`、`src/pipeline/analysis.rs`、
`src/pipeline/planning.rs`；`src/pipeline/mod.rs` 只保留包入口与公共重导出。

```text
raw body + downstream protocol
→ analyze_request
→ RequestRequirements
→ Public Model ordered Routes
→ protocol / capability / limit / reasoning gates
→ RoutePlan<RouteCandidate>
```

`RequestRequirements` 只记录请求事实：public model、协议、streaming、功能组合、输出限制和状态亲和指示。
reasoning level parser 识别 `none`、`minimal`、`low`、`medium`、`high`、`xhigh` 与 `max`；`none` 保持为
显式 level，字段缺失才表示调用方没有请求 reasoning。
`RoutePlan` 固定有序的 Route、Upstream Target 与 Upstream API。Native candidate 通常保留原始 `ApiRequest`；
当该 Upstream API 显式配置 reasoning level 映射时，只在候选请求副本中把 canonical level 改为安全 wire 值。
映射源必须属于有效 Model 的 level 集合，目标值必须满足受限 wire 命名规则，同一源不得重复；未映射候选
保持原始 level，未知下游 level 仍失败关闭。
Bridged candidate 在 egress 前生成受限 `BridgePlan` 与相反协议的 `ApiRequest`。Provider adapter 仍只负责
目标 endpoint、真实 model、header 与认证改写。

请求携带 `previous_response_id` 时，计划关闭跨 target fallback。不同 route 或不同 Upstream API 的能力不会
按字段求并集；一条候选必须独立满足完整请求。

## 6. Provider 适配层

实现位置：`src/provider/kind.rs`、`src/provider/definition.rs`、`src/provider/adapter.rs`、`src/provider/contracts.rs`、
`src/providers/openai_compatible.rs`、各 `src/providers/<provider>.rs` 根模块，以及同名目录中的
`definition.rs` 与可选 `registration.rs`；`src/provider/mod.rs` 和 `src/providers/mod.rs` 只保留包入口，
具体 Provider 不使用 `mod.rs`。

`ProviderKind` 是闭合集合。每个具体 Provider 以一个静态 `ProviderDefinition` 聚合自己的 contract 与 adapter；
`ProviderKind::definition` 是 kind 到具体 definition 的唯一穷举分派，`ProviderKind::contract` 与
`ProviderAdapter::for_kind` 都委托给它。OpenAI、LongCat、OpenRouter、DeepSeek 与 MiMo 的独立静态定义拥有
Provider 契约、endpoint path、request-header hook 与 Responses terminal discriminator；共享 `openai_compatible`
机制负责模型字段改写、Bearer 认证、响应/SSE terminal、
错误分类和 Chat/Responses Upstream API pair 构造。DeepSeek 的 Responses path 缺失时在 adapter 内返回
`UnsupportedProtocol`；OpenRouter 与 MiMo 均声明 Chat/Responses 两个 path。Provider hook 可增添、替换、
转换或删除普通 header；OpenAI 与 LongCat hook 转发 `User-Agent`，OpenRouter hook 不转发可选
attribution/routing header，共享层不维护普通 header allowlist。credential header 在 hook 之后独立附加。credential
pool id、Provider 与 credential kind 来自 `CredentialPoolBinding`，endpoint/timeout 来自 selected Upstream Target。Ingress 按完整
`pool_id + member_id + ProviderKind + CredentialKind` 从 `CredentialStore` 借用 `UpstreamCredential`；每个 Store 条目
冻结 credential type、来源类别、generation 与可选过期时间，来源类别不保存文件路径。Store 不公开通用明文
查询，adapter 仍在 crate 内的认证 header 边界才访问 secret。`CredentialKind` 已能表达
`OAuth2BearerAccessToken`，但现有 Provider contract 仍只允许 `ApiKey`，因此尚未形成 OAuth 出站路径。

每个 Chat/Responses capability 还声明 `ReasoningOutput`：`Unknown` 不表示可读输出，`PlainText` 和 `Summary`
才允许进入方向兼容的 Bridge reasoning channel，`Opaque`（包括 `encrypted_content`）不会被转换。OpenAI、LongCat
与 MiMo 当前都通过共享构造器注册 Chat、Responses 两个独立 Upstream API；每个 Public Model
与它引用的四条 Route 由同一编译注册单元生成，每个下游协议先列 Native route，再列指向相反 Upstream API 的
Bridged route。MiMo 的两个 target 分别绑定 `mimo-v2.5-pro` 与 `mimo-v2.5`，共享 `mimo-primary` pool、
quota scope 与 fault domain。Bridge 生产路径由编译注册表、记录型 transport 与 canonical wire 确定性验证，
但尚未调用真实异构协议 Provider。

OpenRouter 当前注册固定 target `openrouter-nemotron-3-ultra`、Chat/Responses Upstream API 和 Public Model
`nemotron-3-ultra`；两个协议各有唯一 Native route，使用基础 upstream model
`nvidia/nemotron-3-ultra-550b-a55b`。Responses API 的 state affinity 是 `Unbound`，`store`、
`previous_response_id` 与 `background` 在 capability gate 关闭。未注册 Bridged route、fallback 或 `:free` 变体。

DeepSeek 的两个 target 分别绑定 `deepseek-v4-pro` 与 `deepseek-v4-flash`，共享 `deepseek-primary` pool、
quota scope 与 fault domain。每个 target 只注册 Chat Upstream API；Public Model 为 Chat 提供唯一 Native 候选，
为 Responses 提供唯一 Responses→Chat Bridged 候选，不把 Bridge 描述成 Provider 原生 Responses。DeepSeek Chat
的 reasoning output 为 `PlainText`；MiMo 与 LongCat 的 Chat/Responses reasoning output 均为 `Unknown`，因此
它们的 Native-first route 不受影响，但要求可读 reasoning 的 Bridge candidate 会在 egress 前淘汰。

## 7. Transport、SSE、attempt 与 health

实现位置：`src/transport/*` 与 `src/ingress/forwarding.rs`。

共享 `UpstreamClient` 只接收已解析 target 和 adapter 生成的相对 URI，禁止 redirect，并应用 target
timeout。Native streaming response 保持业务 bytes 透明并由 `SseDecoder` 观察 framing/terminal；Bridged
stream 则按完整 event 增量渲染目标协议 wire。下游丢弃任一 body 时，上游 stream 随之取消。
OpenAI-compatible adapter 统一使用 OpenAI terminal 词汇，并把 discriminator 来源作为编译期 Provider
事实：OpenAI/MiMo 从 SSE `event:` 读取，LongCat/OpenRouter 从 data JSON 顶层 `type` 读取。discriminator
不进入 TOML 或运行时探测；双来源 terminal 冲突时失败关闭，也不把尾随 `[DONE]` 代替 Responses 语义终态。

`ingress::attempt::AttemptManager` 管理单请求 attempt 生命周期：stream/non-stream 共享最多 6 次的硬预算，
每候选最多 2 次，attempt 间从 50 ms 起按二倍增长并 capped 到 500 ms；在预算可容纳时为未尝试候选保留
机会。RoutePlan 允许时可进入同一 Public Model 的下一完整候选；下游取消会销毁 pending send、timer 或
response body，提交 response 后不得再拼接另一上游响应。

Ingress 在 response 建立前用 lifecycle guard 捕获 pending send/backoff 取消，建立后把责任移交给外层
`RequestBodyObserver`；后者直接保留 HTTP data/trailer frame，仅在自身提交真实 EOF 或 body error 后报告
end-stream，并在真实 EOF、body error 或 drop 时提交唯一请求终态。response headers ready、首 body 字节与 SSE 首个 text/tool 增量分别计时，避免把 headers ready
误当成 TTFT。JSON usage 只在配置上限内临时解析，SSE usage 按完整 event 解析；业务正文不会写入 tracing 或
进程内累计值。attempt 的 route/target/Provider 等高基数事实只属于 tracing event，`GatewayMetrics` 只维护
进程级低基数单调计数。

`ingress::credential_health::CredentialHealth` 与 `ingress::health::TargetHealth` 在所有 `GatewayState` clone 间共享。
前者维护每 pool round-robin cursor，以及按 `member_id + generation` 隔离的 429 cooldown；`Retry-After`
缺失/非法时为 1 秒并封顶 30 秒。后者只把暂时性 5xx/transport failure 隔离到注册表提供的 `fault_domain`。
无状态请求跳过冷却 member/target；启用 continuation 的 target-bound API 在启动时要求单成员 pool，并保持原
target。两类状态都不持久化、不跨进程，也不执行动态权重或后台探测。

`src/bridge.rs` 作为生产 Protocol Bridge 门面；`bridge/chat.rs` 与 `bridge/responses.rs` 分别维护两种 stream
状态机，`bridge/conversion/request/*`、`response.rs` 与 `stream/*` 分别承担双向请求、非流式响应与增量 SSE
转换。Responses 侧分别固定 response id、item id、call id 和 output index；Chat 侧只用 tool index 关联同一
stream 的分片，不用它替代 call id。两侧要求唯一 terminal 和闭合 JSON object arguments。`BridgePlan` 只接受
显式 allowlist 内的共同 text/function 与明文 reasoning channel 语义；无法表达的字段、opaque continuation 与私有扩展在 egress 前拒绝。

`src/observability.rs` 与 `src/probe.rs` 同样只保留公开门面：前者将 request lifecycle、metrics 与 usage 拆到
同名目录，后者将固定 payload 和受信 probe session 拆到同名目录；各自测试也位于私有 `tests.rs`。

## 8. Probe 与验证层

`openbridge-probe --target <id>` 针对固定 Upstream Target 工作，并按协议选择对应 Upstream API。它复用
target endpoint、adapter 与 transport，只为管理员选中的 target 构造一个上游 pool 快照并确定性使用首个 member；它不
加载下游用户 Key、不接受 URL/model/header/credential 覆盖，也不修改 `RuntimeRegistry`。

测试夹具使用 target/upstream API/route 和 `RequestRequirements + RoutePlan` API。确定性测试保护注册表、
Provider 边界、路由、HTTP/SSE、Bridge、retry/fallback、credential rotation/cooldown、取消与观测行为；它们
不自动升级为外部 SDK、独立 Python/curl、目标 Agent、真实 Provider、负载或长期运行证据。最新实际执行结果
只在[当前实现说明](current-implementation.md#验证状态)维护。

## 9. 尚未实现

- 动态 Converter catalog、route-local 可配置 ConversionPolicy 与异构 Provider 实测；
- 动态 availability/weight、持久化或分布式 cooldown；
- OpenTelemetry/Prometheus exporter、指标 HTTP API、持久化或分布式指标聚合；
- 可安全投影真实 route/upstream API 信息的内部视图与任何扩展 HTTP API；
- Responses WebSocket、OAuth、hosted tool、MCP 和动态 Provider/plugin DSL。

## 关联文档

- [当前实现说明](current-implementation.md)
- [配置、凭证与受信边界](../functional-requirements/configuration-and-credentials.md)
- [路由与 Provider 韧性](../functional-requirements/provider-resilience.md)
