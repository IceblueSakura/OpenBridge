# 当前代码架构

## 状态与边界

本文是当前源码责任和数据流的稳定地图，不维护 Public Model、Target、Provider 能力或测试数量矩阵。客户端行为见
[功能状态](README.md#功能状态)，Provider 当前注册与真实证据见 [Provider 状态](providers/README.md)。配置态公开且具有静态执行候选的
模型以运行中 Models API 为准；该目录不探测 credential、网络、配额或账号状态。

生产注册表使用 `ModelConfig`、`ProviderInstanceConfig`、`CredentialPoolConfig`、`UpstreamTargetConfig`、
`UpstreamApiConfig`、`RouteConfig` 与 `PublicModelConfig`；请求路径使用 operation-specific requirements/plan。
Generation 支持 Native 与显式 Bridge，Embeddings 使用独立 Native-only plan，MCP 本地工具不进入 Provider 链路。

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
optional BridgePlan → ProviderAdapter → trusted Target/API
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
| 上游凭证 | `src/upstream_credentials.rs`、`src/credential/`、`src/oauth2_credentials/` | 私有 binding、purpose-bound secret Store 与 OAuth lifecycle；不拥有 Route |
| Canonical Model | `src/models/` | identity、task、limits、modalities、parameters 与 reasoning 等模型事实 |
| Provider 抽象 | `src/provider/` | 闭合 `ProviderKind`、contract、adapter、错误、credential/header 与 terminal 边界 |
| Provider 实现 | `src/providers/` | trusted origin、operation path、model Target、request hook 与显式 catalog registration |
| Registry | `src/registry/` | 配置引用校验、immutable runtime entity、Public Model DTO/执行快照和编译 |
| Request analysis/planning | `src/pipeline/` | operation-specific facts、preflight 与固定 Route plan；不进行 Provider 名称分支 |
| HTTP ingress | `src/ingress/` | 认证、body lifecycle、handler、attempt/fallback、streaming response 与错误映射 |
| Protocol Bridge | `src/bridge.rs`、`src/bridge/` | Chat ↔ Responses request/response/SSE 转换；不选择 Provider/Route |
| Transport | `src/transport/` | 共享 HTTP client、相对 URI、timeout、safe headers 与 SSE framing |
| Observability | `src/observability.rs`、`src/observability/` | downstream lifecycle、Provider attempt、usage、SDK metrics、OTLP 和本地脱敏 snapshot |
| Probe | `src/probe.rs`、`src/probe/`、`src/bin/openbridge-probe.rs` | 管理员显式选择已注册 Target 的基础观察；不修改 registry |
| MCP | `src/mcp/` | transport/discovery、静态工具目录和逐工具执行；不进入 generation pipeline |

关键 facade 边界：

- `core/capability.rs` 只在 `ApiCapabilities` 汇总域；generation 与 Embeddings 规则分别位于
  `core/capability/generation.rs` 和 `core/capability/embeddings.rs`。
- `pipeline/generation/` 与 `pipeline/embeddings/` 分别拥有 operation analyzer、preflight、planner 和 pure response policy；
  各 analyzer 不解析 registry entity，response policy 不执行 body I/O、observation 或 downstream commit。
- `registry/public_model.rs` 只拥有下游安全 DTO 与 preflight accessor；私有 execution snapshot、contribution、aggregation
  与 Embeddings response budget 由同名子模块拥有。
- `observability.rs` 只作 facade；request/provider/metrics/otlp/http logging 各自拥有对应生命周期。

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
  ├─ RouteConfig[]
  └─ PublicModelConfig[]
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
| `RouteConfig` | Target/API、下游 operation 与 `Native`/`GenerationBridge(direction)` mode |
| `PublicModelConfig` | 下游 identity、reasoning input policy、routing strategy 与有序 source registration |
| `PublicModelInfo` | 下游可序列化模型事实和每 operation 固定 interface；不含执行拓扑 |

编译先验证引用、operation-indexed Provider ceiling、canonical task 与 typed Upstream API key，再验证显式 Generation Bridge direction，
然后从每个固定候选生成 contribution 并保守聚合。Private execution snapshot 由 deterministic
`BTreeMap<OperationKind, ModelExecutionInterface>` 索引；每项同时保存 selected task、typed executable contract、continuation
affinity、operation response budget 与固定顺序 candidates。Candidate 携带完整 `UpstreamApiKey`，forwarding 不再从 Target 与
operation 重建 API identity；JSON/SSE success budget 也从同一个 interface 进入 generation/Embeddings plan。
Public Model 只从 private map 投影固定 Models v1 DTO，并公开全部候选共同保证的能力；请求期不会因能力筛选、跳过或重排 candidate。

Chat 与 Responses 分别使用完整的 operation-specific media envelope。Provider contract 声明 family ceiling，每个 executable Target
必须一次性显式选择 image/audio/file profile；全关闭 default 不复制 ceiling，registration 也不再通过事后清空媒体字段收窄 Target。
Chat `file` 与 Responses `input_file` 使用彼此独立的 typed profile；未实现的 profile 没有公开 constructor，当前所有 Target 均为
`None`。两种 file wire 仍在 analysis 阶段失败关闭，不进入 preflight、Route planning 或 egress。

Route contribution、aggregate 与 private preflight snapshot 通过一个完整 media contract 处理 image/audio/file；Bridge 只贡献
empty media profile。Audio remote URL、data URL 与 pure Base64 source 分别拥有 format/limit payload；request facts 同时保留
per-source 与全 operation cumulative inline budgets。Models v1 继续输出原有 flat audio wire；格式取所有可达 source 的保守交集，
因此不会比 private executable contract 更宽。

Generation media algebra 位于 `core/capability/generation/media.rs`，generation envelope 通过 facade 保持原 crate path。Provider
media ceiling 与 named Target profile 位于同 Provider 的 `media.rs`，不由 model catalog 或 registration 重新定义。

Generation registration 显式选择 `NativeFirst` 或 `SourceFirst`。前者在同一协议先排列所有 Native，再排列 Bridge；后者先
保持 source priority，再在 source 内优先 Native。只有整个 Public Model 缺少某一 downstream protocol Native coverage 时，
compiler 才为允许的单协议 source 自动补充 Bridge；显式 Bridge surface 可独立保留。

Embeddings Public Model 绑定独立 Target/API/Route，不复用 generation target，不进入 `ApiProtocol` 或 Bridge。Public Models DTO
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
→ ProviderAdapter.prepare_request
→ attempt loop / UpstreamTransport
→ Native passthrough or Bridge response renderer
```

Generation 的 pure Chat/Responses analysis、fixed-interface preflight、request normalization、Native/Bridge planning 与
response-mode decision 由 `pipeline/generation/` 单一 family 拥有，并继续通过 pipeline facade 暴露；Bridge plan 不能由
Embeddings family 构造。response driver 根据 success、SSE media、Bridge 和 streaming takeover facts 选择 fail-closed、buffer、
Native SSE validation、Bridge JSON/SSE conversion 或 passthrough；Ingress 执行对应 body read、decoder、observation 和 commit。
该 family 不执行 body read、credential、transport、response-body 或 downstream commit I/O。

Embeddings 的 pure analysis、fixed-interface preflight、Native planning 与 success-response validation 由
`pipeline/embeddings/` 拥有，并继续通过 pipeline facade 暴露原有 API；该 family 不执行 body read、credential、transport、
observation 或 downstream commit I/O。运行路径使用 `EmbeddingRequestRequirements` 与 `EmbeddingRoutePlan`；ingress 在 bounded
read 后调用 pure response driver，并只在验证成功后记录 usage 和构造下游响应。成功 JSON 在首次 commit 前验证 object/index/vector/usage；
没有 Bridge、多 candidate、跨模型 fallback、向量转换、缓存、索引或 tokenizer 估算。

Models list/retrieve 读取同一 immutable Public Model snapshot：标准接口只输出 OpenAI-compatible identity，扩展接口输出下游安全
task/interface/limit/capability。preflight 读取私有 typed contract，不反向解析 Models JSON sentinel。

MCP 在独立 transport/discovery/tool dispatch 中处理：stateless 与 legacy lifecycle 都使用 `POST /mcp`，legacy session 另使用
`GET /mcp` SSE 和 `DELETE /mcp`。当前本地工具不读取 registry、credential 或 Provider transport。

## 6. Provider 与 Transport

`ProviderDefinition` 是静态 contract 与 adapter 的单一入口。OpenAI-compatible family 复用共享 wire machinery，但每个 family
仍显式拥有 origin、Models envelope、operation path、request/header hook、terminal discriminator、credential kind 和模型级 Target。

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
它只在首个下游业务输出前允许有界 local retry 与固定 Route fallback；提交后不得拼接另一上游响应。429 cooldown 按 credential
member/generation 隔离，target fault cooldown 按受信 fault domain 隔离；两者只在单进程内存在，
不持久化、不跨进程，也不执行动态 weight/health probe。

request analysis 把状态要求建模为 typed facts；Public Model compiler 只在所有固定 candidate 对 issuing Target/API/credential
affinity 有共同保证时公开相应 state。opaque continuation 无安全投影时在 egress 前拒绝。

Observability 使用 downstream request root span、Provider attempt child span、固定 routing event allowlist 和 SDK
Counter/Histogram。attributes 只来自固定 allowlist；Public Model 只在 registry planning 后使用，未验证的请求 model、request body、
credential、endpoint URL 和高基数 identity 不进入 OTLP。
metrics 通过 startup-owned OTLP/HTTP exporter 输出，不存在进程内 JSON metrics 查询 API。内容日志只产生本地有界
snapshot，不进入 reviewed OTLP trace layer。

## 8. Probe 与证据边界

`openbridge-probe --target <id>` 只允许已注册且已激活 Target，并复用其 endpoint、adapter、operation 与 credential pool。
它不接受 URL/model/header/credential/body 覆盖，不加载下游用户 key，不修改 `RuntimeRegistry`，也不遍历全部 pool member。
固定观察项只有 Models、最小 Chat、Responses 与 Embeddings；结果只分为 `supported`、`unsupported` 或 `unknown`。

确定性 tests 保护 registry、HTTP/SSE、Provider wire、Bridge、retry/fallback/cooldown、取消和 observability，但不自动升级为外部
SDK、独立 Python/curl、目标 Agent、真实 Provider、负载或长期运行证据。测试资产与实际外部记录分别见
[test-assets](test-assets/inventory.md)和 [evidence](evidence/README.md)。

Generation pure pipeline family 重组通过 Generation ingress/forwarding、Bridge、registry focused contracts，并通过
`cargo fmt -- --check`、`cargo check --locked --all-targets`、`cargo test --locked`、
`cargo clippy --locked -- -D warnings` 与 `git diff --check`；未运行外部 SDK、真实 Provider、负载或长期测试。

Prepared-candidate runner 收敛通过 Embeddings、Generation resilience/OAuth 与 Bridge focused contracts，并通过
`cargo fmt -- --check`、`cargo check --locked --all-targets`、`cargo test --locked`、
`cargo clippy --locked -- -D warnings` 与 `git diff --check`；未运行外部 SDK、真实 Provider、负载或长期测试。

Operation-indexed private execution registry 完成时通过 `cargo fmt -- --check`、`cargo check --locked --all-targets`、
`cargo test --locked`、`cargo clippy --locked -- -D warnings` 与 `git diff --check`；未运行外部 SDK、真实 Provider、负载或长期测试。

## 9. 未实现或未证明

- 动态 Provider/plugin/Route DSL、request-selected endpoint/credential 与在线控制面；
- 动态 availability/weight、持久化或分布式 cooldown；
- 通用异构 conversion policy、完整 OpenAI endpoint/resource catalog 与状态服务；
- 多 Embeddings candidate、Embeddings Bridge、向量转换/缓存/索引/检索和 string tokenizer；
- OTLP logs、内置 Prometheus、指标持久化/查询和分布式 metrics 聚合；
- Responses WebSocket、完整 Agent loop、生产负载与长期运行验收。

## 相关文档

- [实施现状目录](README.md)
- [OpenTelemetry 遥测](telemetry-metrics.md)
- [配置与凭证需求](../functional-requirements/configuration-credentials/README.md)
- [模型能力需求](../functional-requirements/model-capability/README.md)
- [路由与 Provider 韧性](../functional-requirements/routing-resilience/README.md)
