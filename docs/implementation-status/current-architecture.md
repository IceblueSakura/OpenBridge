# 当前代码架构

## 文档职责

本文只维护当前源码责任、依赖方向和数据流，不维护 Public Model/Provider 动态矩阵、测试结果、完成历史或未实现清单。
客户端行为与 Provider 注册见[当前实现](current-state.md)，未实现和未验证范围见[当前状态边界](current-boundaries.md)。

生产注册表使用 `ModelConfig`、`ProviderInstanceConfig`、`CredentialPoolConfig`、`UpstreamTargetConfig`、
`UpstreamApiConfig`、`RouteConfig` 与 `PublicModelConfig`；请求路径使用 operation-specific requirements/plan。
Generation 的 Native 与显式 Bridge 共用canonical Static/Event IR production path，Embeddings 与 Images 使用各自独立的 Native-only plan，MCP 本地工具不进入 Provider 链路。

## 1. 分层与依赖方向

```text
bootstrap + users + private upstream credentials
                    ↓
composition root (`main.rs`)
                    ↓
immutable RuntimeRegistry + UserRegistry + CredentialStore
                    + OAuth2CredentialManager + TelemetryRuntime
                    ↓
HTTP admission / Models projection / MCP transport
                    ↓
operation-specific request facts → Public Model preflight → RoutePlan
                    ↓
canonical Generation plan → ProviderAdapter → trusted Target/API
                    ↓
shared UpstreamTransport → bounded JSON or SSE lifecycle
                    ↓
downstream response + request/attempt observations
```

依赖保持单向：

- 配置、Model、Provider 注册和 registry compilation 不执行网络 I/O；
- request analysis 只提取 wire facts，不解析 registry entity 或选择 Route；
- planning 只消费已经编译的 Public Model interface 和固定候选；
- Provider adapter 不选择 Public Model/Route，也不接受请求指定 endpoint、credential 或认证 header；
- transport 只发送 adapter 生成的受信相对请求，不解释模型能力或业务路由。

## 2. 模块所有权

模块按责任或独立协议域拆分，不按文件行数拆分。facade 保留 crate path，私有子模块承担具体实现。

| 责任域 | 当前 owner | 边界 |
|---|---|---|
| Bootstrap 与进程策略 | `src/config/`、`src/main.rs` | 严格解析、进程启动/关闭和 composition；不拥有 Model/Provider 事实 |
| 下游用户 | `src/identity.rs` | 私有用户表解析、API key 匹配和不可变 `UserRegistry` |
| 上游凭证 | `src/upstream_credentials/`、`src/credential/`、`src/oauth2_credentials/` | 私有 document/binding/materialization/source、purpose-bound secret Store 与 OAuth lifecycle；不拥有 Route |
| Canonical Model | `src/models/` | identity、task、limits、modalities、parameters 与 reasoning 等模型事实 |
| Provider 抽象 | `src/provider/` | 闭合 `ProviderKind`、contract、adapter、错误、credential/header 与 terminal 边界 |
| Provider 实现 | `src/providers/` | trusted origin、operation path、model Target、request hook 与显式 catalog registration |
| Registry | `src/registry/` | 配置引用校验、immutable runtime entity、Public Model DTO/执行快照和编译 |
| Request analysis/planning | `src/pipeline/` | operation-specific facts、preflight 与固定 Route plan；不进行 Provider 名称分支 |
| Generation semantic IR | `src/ir/generation/` | pure Static/Event values、reducer/materializer、local validation、semantic requirements与fidelity；不拥有Registry、I/O或routing |
| HTTP ingress | `src/ingress/` | 认证、body lifecycle、handler、attempt/fallback、streaming response 与错误映射 |
| Generation codecs | `src/bridge.rs`、`src/bridge/static_codec/`、`src/bridge/event_codec/` | production Native PreserveSource与Chat ↔ Responses request/response/SSE lowering；只消费固定Route与显式budgets，不选择Provider、credential、URL或commit policy |
| Transport | `src/transport/` | 共享 HTTP client、相对 URI、timeout、safe headers 与 SSE framing |
| Observability | `src/observability.rs`、`src/observability/` | downstream lifecycle、Provider attempt、usage、SDK metrics、OTLP 和本地脱敏 snapshot |
| Probe | `src/probe.rs`、`src/probe/`、`src/bin/openbridge-probe.rs` | 管理员在已注册 Generation Target 边界内执行 Models 与 candidate-model Generation 矩阵；不修改 registry |
| MCP | `src/mcp/` | transport/discovery、静态工具目录和逐工具执行；不进入 generation pipeline |

关键 facade 边界：

- `core/capability.rs` 只在 `ApiCapabilities` 汇总域；Generation、Embeddings 与 Images 规则分别位于
  `core/capability/generation.rs`、`core/capability/embeddings.rs` 和 `core/capability/images.rs`。
- `pipeline/generation/`、`pipeline/embeddings/` 与 `pipeline/images/` 分别拥有 operation types/errors、analyzer、preflight、planner 和 pure response policy；
  各 analyzer 不解析 registry entity，response policy 不执行 body I/O、observation 或 downstream commit。
- `registry/public_model.rs` 是稳定 facade；operation DTO、media algebra、private execution snapshot、contribution、aggregation
  与 operation response budget 由 `public_model/*` owner 持有。
- `observability.rs` 只作 facade；request terminal、request content snapshot、provider、metrics、otlp 与 http JSONL 各自拥有对应生命周期。

## 3. 启动装配

启动链路：

```text
BootstrapConfigPath::load
→ TelemetryRuntime::from_bootstrap + init_tracing
→ UserConfigPath::load
→ UpstreamCredentialConfigPath::load
→ derive active credential-pool identities
→ providers::build_compiled_registry_with_active_pools
→ build CredentialStore + OAuth2CredentialManager
→ validate registry/credential ownership
→ UpstreamClient + GatewayState
→ ingress::build_router
→ axum::serve
→ bounded telemetry shutdown
```

`BootstrapConfig` 拥有 listener、私有配置 locator、共享 HTTP client、body/replay/SSE budget、项目默认 instructions、
本地下游内容日志开关和 OTLP exporter base。Provider、Model、Target、API、Route 与 Public Model 由受信 Rust catalog 编译；
TOML 不能新增这些实体。

active pool 集合只能禁用引用未激活凭证的 Target，不能新增 Provider、endpoint、Route 或能力。普通用户/API-key 配置不热重载；
独立 login CLI 可以创建完整的 OpenBridge-owned auth 文件，主服务则要求启动时已能读取并校验该文件。运行中的 OAuth manager 只在
到期 refresh 或受控 401 recovery 内 reload/rotate 同一文件。所有解析、引用和 credential ownership 错误都在 listener 绑定前失败。

## 4. Registry 与 Public Model

```text
RegistryConfig
  ├─ ModelConfig[]
  ├─ ProviderInstanceConfig[]
  ├─ CredentialPoolConfig[]
  ├─ UpstreamTargetConfig[]
  │    └─ UpstreamApiConfig[]
  └─ PublicModelConfig[]
       └─ ordered RouteConfig[]
         ↓ validate + compile
immutable RuntimeRegistry
```

| 实体 | 所有事实 |
|---|---|
| `ProviderContract` | operation-indexed adapter 能力上界与允许的 credential kind |
| `ProviderInstanceConfig` | 稳定实例 ID、闭合 `ProviderKind` 与唯一受信 base URL |
| `ModelConfig` | canonical identity 与必填 task variant；task payload 独占相关 limits/modalities/parameters/reasoning |
| `CredentialPoolConfig` | 非敏感 pool identity、Provider 与 credential kind |
| `UpstreamTargetConfig` | Provider instance、canonical/provider model、credential pool、timeout、quota/fault domain 与 API 列表 |
| `UpstreamApiConfig` | typed `(operation, task)` key、upstream model、executable capability、streaming policy 和模型级收窄 |
| `RouteConfig` | Public Model 私有的 Target/API 与下游 operation；不拥有 ID，mode 由 upstream/downstream operation pair 派生 |
| `PublicModelConfig` | 下游 identity、reasoning input policy 与有序 typed Route candidates |
| `PublicModelInfo` | 下游可序列化模型事实和每 operation 固定 interface；不含执行拓扑 |

编译先验证引用、operation-indexed Provider ceiling、canonical task 与 typed Upstream API key，再从 operation pair 派生 Native 或显式 Generation Bridge direction；跨协议 pair 仅允许 canonical Generation task，非法 pair 和重复结构候选在启动时失败。编译随后从每个固定候选生成 contribution 并保守聚合。Private execution snapshot 由 deterministic
`BTreeMap<OperationKind, ModelExecutionInterface>` 索引；每项同时保存 selected task、typed executable contract、continuation
affinity、operation response budget 与固定顺序 candidates。Candidate 携带完整 `UpstreamApiKey`，forwarding 不再从 Target 与
operation 重建 API identity；JSON/SSE success budget 也从同一个 interface 进入 Generation、Embeddings 或 Images plan。
Public Model 只从 private map 投影固定 Models v1 DTO，并公开全部候选共同保证的能力；请求期不会因能力筛选、跳过或重排 candidate。

Chat 与 Responses 分别使用完整的 operation-specific media envelope。Provider contract 声明 family ceiling，每个 executable Target
必须一次性显式选择 image/audio/file profile；全关闭 default 不复制 ceiling，registration 也不再通过事后清空媒体字段收窄 Target。
Chat `file` 与 Responses `input_file` 使用彼此独立的 typed profile；analysis 冻结 source/encoding/PDF detail 与 bounded resource facts，
private preflight 读取同源 Models interface，Native route 保持 wire，Bridge 与无 affinity `file_id` fail closed。OpenAI family ceiling 描述
标准 API wire 上限，但当前所有 checked-in executable Target 仍显式选择 `None`，所以生产 Public Models 不公开 file。

Route contribution、aggregate 与 private preflight snapshot 通过一个完整 media contract 处理 image/audio/file；Bridge 只贡献
empty media profile。Audio remote URL、data URL 与 pure Base64 source 分别拥有 format/limit payload；request facts 同时保留
per-source 与全 operation cumulative inline budgets。Models v1 继续输出原有 flat audio wire；格式取所有可达 source 的保守交集，
因此不会比 private executable contract 更宽。

Generation media algebra 位于 `core/capability/generation/media/` 的 audio/image/file leaves，generation envelope 通过 facade 保持原 crate path。Provider
media ceiling 与 named Target profile 位于同 Provider 的 `media.rs`，不由 model catalog 或 registration 重新定义。

Generation registration 显式选择 `NativeFirst` 或 `SourceFirst`。前者在同一协议先排列所有 Native，再排列 Bridge；后者先
保持 source priority，再在 source 内优先 Native。只有整个 Public Model 缺少某一 downstream protocol Native coverage 时，
compiler 才为允许的单协议 source 自动补充 Bridge；显式 Bridge surface 可独立保留。

Embeddings Public Model 直接拥有独立 Target/API candidate，不复用 generation target，不进入 `ApiProtocol` 或 Bridge。Public Models DTO
永不序列化 Provider、Target、Route、upstream model、endpoint、credential、健康状态或价格。

## 5. HTTP 与请求规划

Router 先执行 request ID、body budget、Bearer authentication 和安全 tracing，再进入 operation handler。匿名失败不会触发
内容 snapshot；业务正文只有 bootstrap 显式开启时才在最终下游边界做有界、本地、强制脱敏的 info event。

Generation 请求路径：

```text
JSON admission
→ analyze RequestRequirements
→ resolve Public Model + operation interface
→ capability/state/limit preflight once
→ normalize shared request policy once
→ expand fixed RouteCandidate list
→ optional Bridge request conversion
→ GenerationProviderAdapter.prepare_routed_request
→ attempt loop / UpstreamTransport
→ Native passthrough or Bridge response renderer
```

Generation 的 pure Chat/Responses analysis、fixed-interface preflight、request normalization、Native/Bridge planning 与
response-mode decision 由 `pipeline/generation/` 单一 family 拥有，并继续通过 pipeline facade 暴露；Bridge plan 不能由
Embeddings family 构造。response driver 根据 success、SSE media、Bridge 和 streaming takeover facts 选择 fail-closed、buffer、
Native SSE validation、Bridge JSON/SSE conversion 或 passthrough；Ingress 执行对应 body read、decoder、observation 和 commit。
成功的 Native/Bridge SSE 在首个完整、Provider-valid 且下游可见的 event 前仍由 attempt runner 持有：first-event timeout 或 body
transport failure 可以走既有有界 retry/fallback，非法 framing 或 event 前 EOF 则在未 commit 时返回安全 `502`。首 event 后不再
retry/fallback；terminal 前 EOF 保留已经可见的字节并以 downstream body error 结束，不伪造 terminal。precommit 只保留受
`max_sse_event` 限制的单个 raw event：Native 原样 replay；Bridge 对不可见 event 立即释放 raw bytes，并把已推进的
renderer、首段转换输出、event-idle deadline 与同一网络 chunk 的剩余字节一起 hand off 给 postcommit body owner。
该 family 不执行 body read、credential、transport、response-body 或 downstream commit I/O。

Embeddings 的 pure analysis、fixed-interface preflight、Native planning 与 success-response validation 由
`pipeline/embeddings/` 拥有，并继续通过 pipeline facade 暴露原有 API；该 family 不执行 body read、credential、transport、
observation 或 downstream commit I/O。运行路径使用 `EmbeddingRequestRequirements` 与 `EmbeddingRoutePlan`；ingress 在 bounded
read 后调用 pure response driver，并只在验证成功后记录 usage 和构造下游响应。成功 JSON 在首次 commit 前验证 object/index/vector/usage；
没有 Bridge、多 candidate、跨模型 fallback、向量转换、缓存、索引或 tokenizer 估算。

Images 的 strict analysis、fixed-interface preflight、优先级 candidate planning 与 pure success-response projection 由
`pipeline/images/` 拥有。多个固定 candidate 的 Public interface 是保守交集；request-time 只选择配置顺序第一项并执行一次，
不 retry、fallback 或 rotate credential。`ingress/forwarding/images.rs` 拥有单 attempt、bounded body read、typed terminal、
validated-only usage 与 downstream commit；image URL/bytes 不进入普通 OTLP attributes。

Models list/retrieve 读取同一 immutable Public Model snapshot：标准接口只输出 OpenAI-compatible identity，扩展接口输出下游安全
task/interface/limit/capability。preflight 读取私有 typed contract，不反向解析 Models JSON sentinel。

MCP 在独立 transport/discovery/tool dispatch 中处理：stateless 与 legacy lifecycle 都使用 `POST /mcp`，legacy session 另使用
`GET /mcp` SSE 和 `DELETE /mcp`。当前本地工具不读取 registry、credential 或 Provider transport。

## 6. Provider 与 Transport

`ProviderDefinition` 是静态 contract 与 adapter 的单一入口。OpenAI-compatible family 复用共享 wire machinery，但每个 family
仍显式拥有 origin、Models envelope、operation path、request/header hook、terminal discriminator、credential kind 和模型级 Target。
共享实现由 `src/providers/openai_compatible/` facade 按 surface、request、header、response 与 registration 责任聚合，并保持
`providers::openai_compatible` crate-internal 路径稳定。
请求准备前先按 `OperationKind` 从 definition 选择 closed typed adapter：Generation adapter 固定 Chat Completions 或 Responses，
Embeddings/Images adapter 不能调用 Generation request/SSE policy。请求 body/protocol 不能隐式切换 operation；Provider headers、authentication、
status classification 与 model-list probe 仍通过同一 operation-neutral adapter 共享。`provider/adapter.rs` 拥有 common policy，
`provider/operation.rs` 从同一静态 surface 原子选择 relative path、capability ceiling 与 typed request/SSE policy。
数据面 Generation 与 Embeddings/Images preparation 都必须接收 operation-matched `UpstreamApi`。独立管理员 probe 另有窄化的 Generation
preparation：只接受校验后的 model ID 和固定合成请求，仍由已注册 Target、Provider operation path/body hook、credential 与 timeout
约束，拒绝借非 Generation Target 扩大 operation，且不套用另一已注册模型的 ignored-parameter 或 reasoning mapping；它不被 ingress
或业务请求调用。Generation probe 默认保留固定 16-token upstream output limit；只有显式风险开关可为 streaming request 省略。

普通安全 header 与认证 header 分离。业务请求不能控制上游 URL、Provider、Target、credential、认证 header、代理 header 或
转换脚本。`UpstreamClient` 只接受已解析 Target 和 adapter 生成的相对 URI，禁止 redirect，并应用 target timeout。

ChatGPT 是固定 Responses-only Codex backend profile：adapter 固定 `Accept: text/event-stream`、
`originator: codex_cli_rs` 和 headless Linux UA `codex_cli_rs/0.146.0 (Linux unknown; x86_64) unknown`，不读取宿主
OS、terminal identity 或本机 Codex auth。它要求上游 stream，并允许经过验证的缺失 success Content-Type 作为 SSE；其他
Provider 不继承该例外。

Native SSE 业务 bytes 保持透明，由 decoder 观察 framing/terminal；Bridge SSE 按完整 event 增量渲染。需要 bounded takeover 的
API 在下游 commit 前完整校验上游 SSE，并生成非流式 JSON。非法 media、超限、UTF-8/framing/terminal 冲突或 EOF-before-terminal
失败关闭；下游 drop 会取消对应上游 body。

## 7. Retry、状态与 Observability

顶层 `execution::AttemptCoordinator` 只拥有 request/candidate attempt counts、固定 hard limits、retry/fallback step 与 capped backoff；
Generation 和 Embeddings forwarding 共用该 state machine，operation pipeline、Provider 分类、credential 选择与 downstream commit 不进入 coordinator。
Prepared Generation 与 Embeddings candidates 通过 `ingress/forwarding/execution/runner.rs` 的单一 send/retry loop 执行；
closed `OperationDriver` 只分派 OAuth/replay/health/terminal policy，不读取 Public DTO、Provider 名称或重新选择 Route。
Images 在 operation handler 中以 finish-only 方式复用同一 coordinator 与 attempt observation，但不进入 replay runner。
Generation/Embeddings runner 只在首个下游业务输出前允许有界 local retry 与固定 Route fallback；提交后不得拼接另一上游响应。429 cooldown 按 credential
member/generation 隔离，target fault cooldown 按受信 fault domain 隔离；两者只在单进程内存在，
不持久化、不跨进程，也不执行动态 weight/health probe。

request analysis 把状态要求建模为 typed facts；Public Model compiler 只在所有固定 candidate 对 issuing Target/API/credential
affinity 有共同保证时公开相应 state。opaque continuation 无安全投影时在 egress 前拒绝。

Observability 使用 downstream request root span、Provider attempt child span、固定 routing event allowlist 和 SDK
Counter/Histogram。attributes 只来自固定 allowlist；Public Model 只在 registry planning 后使用，未验证的请求 model、request body、
credential、endpoint URL 和高基数 identity 不进入 OTLP。
metrics 通过 startup-owned OTLP/HTTP exporter 输出，不存在进程内 JSON metrics 查询 API。内容日志只产生本地有界
snapshot，不进入 reviewed OTLP trace layer。
