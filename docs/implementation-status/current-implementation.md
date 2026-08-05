# 当前实现说明

## 状态与范围

本文只记录当前可运行入口、外部行为、Provider 注册和验证状态。模块分层、类型职责与内部数据流统一见
[当前代码架构](current-architecture.md)。OpenBridge 仍是实验性原型；每次记录分别说明确定性测试、真实 Provider 与未运行的验收层，
不能把其中一层的结果外推为外部 SDK、负载、长期运行或生产兼容。

## 当前运行入口

默认启动：

```bash
cp config/users.example.toml config/users.toml
cp config/upstream-credentials.example.toml config/upstream-credentials.toml
# 编辑两份私有 TOML；填写用户/API key，并按需启用注释中的 ChatGPT auth_json_file。
cargo run --bin openbridge --locked
```

schema v2 的 `bootstrap.toml` 包含 loopback listener、两份私有 credential 文件位置、彼此独立的 request/JSON
response/replay/SSE 上限和共享 HTTP client 参数。Provider family、Provider instance、Model、Upstream Target、Upstream API、
Route、Public Model 和 credential pool binding 均由 Rust 代码注册；Provider instance 唯一拥有 endpoint BaseURL。修改后需要重新编译
或重启。

运行配置与模板一一对应：`config/bootstrap.toml` 使用 `config/bootstrap.example.toml`，
`config/users.toml` 使用 `config/users.example.toml`，`config/upstream-credentials.toml` 使用
`config/upstream-credentials.example.toml`。

| Endpoint                              | 当前行为                                                                    | 认证        |
|---------------------------------------|-----------------------------------------------------------------------------|-------------|
| `GET /healthz`                        | 返回 `status` 与 `registry_version`                                         | 无          |
| `GET /openapi.yaml`                   | 返回当前构建内置的 OpenAPI 3.0.3 YAML                                       | 无          |
| `GET /swagger-ui`、`GET /swagger-ui/` | 返回用于本地接口测试的 Swagger UI 页面                                      | 无          |
| `GET /v1/models`                      | 返回代码注册 Public Model 的 OpenAI 标准四字段列表                          | 静态 Bearer |
| `GET /v1/models/{model}`              | 返回一个 OpenAI 标准四字段 Model 对象                                       | 静态 Bearer |
| `GET /openbridge/v1/models`           | 返回 Public Model 模型事实与固定接口能力列表                                | 静态 Bearer |
| `GET /openbridge/v1/models/{model}`   | 返回一个完整 Public Model 能力对象                                          | 静态 Bearer |
| `POST /v1/chat/completions`           | 按完整 Route 执行 Chat Native 或 Chat→Responses Bridge 的 JSON/SSE          | 静态 Bearer |
| `POST /v1/responses`                  | 按完整 Route 执行 Responses Native 或 Responses→Chat Bridge 的 JSON/SSE     | 静态 Bearer |
| `POST /v1/embeddings`                 | 按独立固定接口执行单条 Native Route，并在下游 commit 前校验有界 JSON 成功体 | 静态 Bearer |

OpenAPI 规范源文件为 [`docs/openapi.yaml`](../openapi.yaml)，Swagger UI 页面源文件为
[`docs/swagger-ui.html`](../swagger-ui.html)。两项文档 endpoint 都是静态资源，不读取 Provider、 Upstream Target 或
credential；Swagger UI 的业务请求仍由既有 Bearer 认证 middleware 保护。

下游用户和 API Key 来自启动时读取的私有 `config/users.toml`。私有 `config/upstream-credentials.toml` 的每个编译期 binding
现在只能在有序 `api_keys` 与单一 `auth_json_file` 中二选一；Provider 与 credential kind 仍只从代码注册表解析。服务与普通
API-key probe 不读取上游 key 环境变量或 `.env`，普通 probe 也不会打开未选中的 OAuth locator。服务在 listener 绑定前把已启用用户
Key 与全部启用 target 引用的 API-key pool 合并为不可变 `CredentialStore`，并把所有显式配置的 ChatGPT OAuth2 文件校验为完整
token bundle 后装入独立的不可变 `OAuth2CredentialManager`。未知或重复 binding、source/kind 错配、同 Provider 多 auth 文件、
损坏 TOML、无效 API-key pool 或损坏/过期 OAuth2 bundle 会阻止启动。运行时不重新读取 TOML 或 auth 文件；两份快照均不提供热更新。
Store 和 manager 条目只公开非敏感 identity/metadata；文件路径不进入诊断元数据或 `Debug`。

第六个 ChatGPT Provider 注册一个默认禁用且没有 Route/Public Model 的 OAuth pool/target。ChatGPT credential 只从 private upstream
TOML 显式配置的 OpenBridge-owned `auth_json_file` 进入独立启动快照；当前没有本机 Codex auth loader、OS/environment/terminal
identity 探测或专用 ChatGPT probe。当前仍没有 token 获取、PKCE/device login、refresh、数据面接入、热更新或 401
refresh/retry 行为。

### 2026-08-05 `src` 模块职责收敛

- 对 `src` 全量模块树、根模块重导出、主要类型/函数、文件体量和跨层数据流完成结构审计；按“独立变化原因”而不是行数识别出三个混合职责根文件，
  其余较大的 Bridge/SSE 状态机、Credential Store、Observability、Provider adapter 与 registry validation/compiler 文件仍保持单一责任，
  未做机械拆分；
- `core/capability.rs` 现在只作为 capability facade 并组合 `ApiCapabilities`；Chat/Responses generation 与 Embeddings 的字段、校验和
  subset 规则分别位于 `core/capability/generation.rs`、`embeddings.rs`；
- `pipeline/analysis.rs` 现在只重导出 operation-specific analyzer；Chat/Responses 请求事实与严格 Embeddings request union 分别位于
  `pipeline/analysis/generation.rs`、`embeddings.rs`，二者仍不查询 registry、不选择 Route、不改写 body；
- `registry/public_model.rs` 只保留下游安全 DTO 与 preflight accessor；私有 execution interface/candidate、编译编排、Route 契约贡献/保守
  交集和 Embeddings response budget 分别进入 `public_model/execution.rs`、`compiler.rs`、`compiler/contract.rs` 与
  `compiler/embedding_budget.rs`。Registry 总编译器仍经原 facade 调用，公共 Models serialization 不包含执行拓扑；
- `openbridge::core::*`、`openbridge::pipeline::*`、`openbridge::registry::*` 的既有重导出和全部运行行为保持不变。功能需求将物理文件布局从
  产品契约中移除，当前模块树与维护约束分别同步到当前代码架构和 `AGENTS.md`。

改动前后实际执行并通过：

```text
cargo test --locked
cargo check --locked
cargo fmt -- --check
cargo test --locked --test capability_definition_contract --test embedding_definition_contract --test embedding_registry_contract --test native_routing_contract
cargo test --locked
cargo clippy --locked -- -D warnings
git diff --check
```

本轮没有修改 `testdata/` 或 `tools/corpus/`，因此未运行 Python corpus baseline；两个外部客户端测试保持 ignored。未运行外部 SDK、真实
Provider、Agent runtime、负载或长期验收；本次结构 refactor 不把确定性回归提升为这些外部兼容证据。

### 2026-08-05 OpenBridge-owned OAuth2 启动快照

- upstream credential schema v1 的每个 `credential_pools` 项现在是互斥 union：API-key binding 使用有序 `api_keys`，OAuth2 binding
  使用单一 `auth_json_file`；registry 仍决定 Provider 与 credential kind，同一个 OAuth2 Provider 不能绑定多个文件；
- `OAuth2CredentialManager` 在启动时读取 OpenBridge-owned Codex-compatible ChatGPT JSON，要求显式 ChatGPT auth mode、完整且非空的
  id/access/refresh token、未过期 access-token expiry、一致的 account binding 与非空的可选 `last_refresh`，然后保留不可变且
  `Debug` 脱敏的单 Provider 快照；
- 相对 locator 以 upstream TOML 的目录为基准。真实进程 composition test 已证明 OAuth2 文件在 listener 绑定前加载；启动后修改源文件
  不会改变 manager。普通 API-key probe 只打开选中的 API-key source，不读取同一 TOML 中未选中的 OAuth2 locator；
- manager 已进入 `GatewayState`，但 ChatGPT target 仍默认禁用且没有 Route/Public Model。当前 manager 没有 refresh、reload、持久化、
  后台任务或 401 recovery API；这些仍属于下一焦点；
- `tests/example_config.rs` 的 DeepSeek V4 Pro reasoning level 断言已与当前模型定义同步为 `Max, High`，没有修改模型配置。

本轮实际执行并通过：

```text
cargo test --locked --lib oauth2_credentials
cargo test --locked --test upstream_credential_config
cargo test --locked --test startup_contract
cargo test --locked --test example_config compiled_model_catalog_includes_litellm_text_models
cargo test --locked
cargo clippy --locked -- -D warnings
git diff --check
```

完整 Rust 测试通过，两个显式外部客户端测试保持 ignored。聚焦修改的 OAuth2 manager、upstream credential、startup 与 state/main Rust
文件通过单文件 `rustfmt --check`；仓库级 `cargo fmt -- --check` 在当前 rustfmt 1.9.0 下仍因大量本轮未修改源码的既有格式差异失败，
本轮没有机械改写这些无关文件。未使用真实 auth 文件或真实 Provider；未运行外部 SDK、PKCE/device login、refresh、负载或长期运行验收。

### 2026-08-05 Upstream API 协议事实去重

- `UpstreamApiConfig` 不再保存可与 capabilities variant 冲突的 `operation`，Runtime `UpstreamApi::operation()` 直接从
  `UpstreamApiCapabilities` 派生；
- 当前 operation 已唯一固定 JSON/SSE generation 或有界 JSON Embeddings transport，因此 Config/runtime 删除独立 transport 与
  mismatch error；
- 无执行消费者的 endpoint profile 字符串体系已从 Upstream API、Provider contract、compiler 和 runtime 删除。Provider adapter
  继续静态拥有 operation path，Provider instance BaseURL 与 credential/header 受信边界保持不变；
- TDD 先让 Embeddings definition 因省略旧字段而编译失败；实现后聚焦 definition/Provider tests、完整 `cargo test --locked`、
  `cargo fmt -- --check`、`cargo clippy --locked -- -D warnings` 与 `git diff --check` 均通过。两个外部客户端测试保持 ignored；未运行
  真实 Provider、负载或长期运行验收。

### 2026-08-05 typed Upstream API 身份

- `UpstreamApiConfig` 不再声明字符串 `id`；Target 以 `BTreeMap<OperationKind, UpstreamApi>` 保存 API，并在启动编译阶段拒绝同一
  Target 重复 operation；
- Route、Public Model 预编译 candidate、request plan、forwarding、probe 与 continuation issuer 统一使用 typed upstream
  operation。Native/Bridge 校验、候选顺序、state affinity 与 trusted-egress 行为未改变；
- Provider attempt telemetry 将旧 `upstream_api` 维度替换为稳定低基数的 `upstream_operation`，并继续单独保留下游
  `operation`；
- TDD 先证明旧 ID 索引把 duplicate operation 延迟成无关的 Native Route mismatch；实现后聚焦 registry/routing/probe/
  forwarding/observability/Embeddings tests、完整 `cargo test --locked`、`cargo fmt -- --check`、
  `cargo clippy --locked -- -D warnings` 与 `git diff --check` 均通过。两个外部客户端测试保持 ignored；未运行真实 Provider、负载或
  长期运行验收。

### 2026-08-05 独立 Provider 实例注册

- 新增 `ProviderInstanceConfig { id, kind, base_url }` 与 runtime `ProviderInstance`；compiler 在 Target 之前建立唯一实例索引，统一校验
  空 ID、重复 ID 与受信 HTTPS BaseURL；
- `UpstreamTargetConfig` 删除重复的 Provider kind 和 BaseURL，只引用 `provider_instance`。Runtime Target 持有已解析实例并从它取得
  adapter kind 与 endpoint；credential pool Provider ownership、capability ceiling 和 trusted-egress 校验仍 fail closed；
- 六个 built-in Provider family 各自显式注册当前部署实例。Registry 允许同一 `ProviderKind` 注册多个不同实例；测试用两个 OpenAI
  实例证明不同 URL/区域无需在一个实例或 Target 内引入 URL 列表；
- TDD 先让 synthetic registry 因不存在 `ProviderInstanceConfig`、`RegistryConfig.provider_instances` 和 Target 引用字段而编译失败；
  实现后聚焦 config/catalog/routing/probe/forwarding/credential tests、完整 `cargo test --locked`、`cargo fmt -- --check`、
  `cargo clippy --locked -- -D warnings` 与 `git diff --check` 均通过。两个外部客户端测试保持 ignored；未运行真实 Provider、外部 SDK、
  负载或长期运行验收。

## Provider 与请求行为

闭合 `ProviderKind` 当前包含 OpenAI、LongCat、OpenRouter、DeepSeek、Xiaomi MiMo 与 ChatGPT，六者都进入 compiled
registry。ChatGPT 只形成默认禁用的隔离 target；当前可路由目录与该隔离注册项如下，“Bridge 候选”只表示已注册的协议转换路径，
不表示上游原生支持该协议：

| Provider              | Public Model                 | 固定 Upstream Target                                | 下游可用 Route surface                                                | Credential pool                          |
|-----------------------|------------------------------|-----------------------------------------------------|-----------------------------------------------------------------------|------------------------------------------|
| OpenAI                | `gpt-5.6-sol`                | `openai-main`                                       | Chat/Responses Native-first，各有指向相反 Upstream API 的 Bridge 候选 | `openai-primary`                         |
| OpenAI                | `text-embedding-3-small`     | `openai-text-embedding-3-small`                     | `embeddings` Upstream API 的唯一 Native Route；无 Bridge/fallback     | `openai-primary`                         |
| LongCat               | `LongCat-2.0`                | `longcat-2`                                         | Chat/Responses Native-first，各有指向相反 Upstream API 的 Bridge 候选 | `longcat-primary`                        |
| OpenRouter            | `deepseek-v4-flash`          | `openrouter-deepseek-v4-flash`                      | Chat 与无状态 Responses 各一条 Native Route；无 Bridge                | `openrouter-primary`                     |
| DeepSeek              | `deepseek-v4-pro`            | `deepseek-v4-pro`                                   | Chat Native；无 Responses 接口                                        | `deepseek-primary`                       |
| DeepSeek + OpenRouter | `deepseek-v4-flash`          | `deepseek-v4-flash`、`openrouter-deepseek-v4-flash` | DeepSeek Chat Native；OpenRouter Chat/Responses Native                | `deepseek-primary`、`openrouter-primary` |
| Xiaomi MiMo           | `mimo-v2.5-pro`、`mimo-v2.5` | `mimo-v2-5-pro`、`mimo-v2-5`                        | Chat/Responses Native-first，各有指向相反 Upstream API 的 Bridge 候选 | `mimo-primary`                           |
| ChatGPT subscription  | 无                           | `chatgpt-gpt-5-6-sol`（默认禁用）                   | 无 Route/Public Model；通用 probe 拒绝禁用 target                     | `chatgpt-codex`（OAuth；可选启动快照）   |

代码目录的 generation Public Model registration 现在持有有序 Provider route source 列表；对每个下游协议，编译器先按 source
声明顺序生成全部 Native Route，再按相同顺序生成 Bridge Route。相同 canonical Model ID 不会自动注册 或加入候选。上表中
`deepseek-v4-flash` 显式拥有两个 Provider source；其他 checked-in generation Public Model 目前各只有一个 source。 跨
Provider fallback 仍严格遵循该 Public Model 的固定 source 顺序。 聚合 Responses Route 的 `previous_response_id` 还要求全部可执行
Route 唯一绑定同一个 Target/operation；多个潜在 签发者即使各自声明支持，也会在固定公共契约中收窄为 `unsupported` 并移出接口参数列表，避免无
issuer ledger 时把 continuation ID 盲投首选 Provider。唯一 issuer 的多个 Route 仍可形成契约，但请求执行只使用第一候选。

OpenRouter 的 `store`、`previous_response_id` 与 `background` 能力关闭，也未注册 `:free` 变体。五个可路由 Provider family 分别拥有
独立静态 definition、Provider instance、upstream model 与能力，并采用 OpenAI-compatible wire。ChatGPT definition 只开放 OAuth2
Bearer、固定 Codex backend Provider instance 与 Responses wire profile；它不是 Public Model 数据面，没有当前可执行 probe，也不构成
通用异构 wire Provider。

MiMo 的 `mimo-v2.5-pro` 与 `mimo-v2.5` Chat/Responses Native Upstream API 均声明支持
`parallel_tool_calls`、image input 和 structured output；两种协议的 `store` 均关闭，Responses 的
`previous_response_id` 与 `background` 均关闭。两种协议的 `reasoning_output` 保持 `Unknown`，因此这组声明只 控制请求能力
gate 和 Native 原样转发，不证明 Provider 会输出可读 reasoning，也不扩大反向 Bridge 的转换能力。

六个具体 Provider 均以静态 `ProviderDefinition` 聚合自身 contract 与 adapter；
`ProviderKind::definition` 是唯一穷举分派，现有 contract 与 adapter 查询接口都委托给该描述符。 descriptor 不注册
target、Route 或 Public Model，也不读取 endpoint origin 或 credential。

canonical 模型目录当前包含 16 个定义：15 个 generation 模型和独立的
`openai/text-embedding-3-small` Embedding 模型。其中 generation 模型覆盖 GPT-5.6/5.5/5.3 Codex Spark、 DeepSeek V4、MiMo
V2.5、Qwen3.7、GLM-5.2、Kimi K3 和 MiniMax M3；已确认的 context、输出上限、参数、reasoning 状态和 level 保存在各自模型模块。GPT-5.6
Sol、LongCat 2.0、两个 DeepSeek V4 和两个 MiMo V2.5 模型已被固定 target 与 Public Model 引用；`deepseek-v4-flash` 由
DeepSeek 与 OpenRouter 两个 target 共享同一个 canonical model，
`text-embedding-3-small` 由独立 OpenAI target 引用。其余目录项尚未新增 Provider target 或 Public Model route， 不构成真实可调用声明。
`ModelConfig` 已能表达 Embedding，但仍没有 rerank task。

2026-08-05 调整 checked-in 模型与 Provider 路由矩阵：

- Public Model `code-primary` 和 `embedding-primary` 分别改为实际模型名 `gpt-5.6-sol` 与
  `text-embedding-3-small`；对应的 OpenAPI、README、Embeddings contract 和 Models/forwarding 测试已同步。
- OpenRouter 移除 Nemotron target/public route，改为 `openrouter-deepseek-v4-flash`，以
  `deepseek/deepseek-v4-flash` 提供 Chat/Responses Native；`deepseek-v4-flash` Public Model 显式聚合 DeepSeek Chat 与
  OpenRouter Chat/Responses，保留固定 source 顺序。
- DeepSeek 两个直连 target 均保持 Chat-only；`deepseek-v4-pro` 不再生成 Responses Bridge，`deepseek-v4-flash`
  的 Responses 只通过 OpenRouter Native route 可用。canonical catalog 同时移除 `tencent/hy3` 与不再绑定的
  `nvidia/nemotron-3-ultra-550b-a55b` 模型定义及模块。
- `cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings` 和 `git diff --check`
  均通过；全量 Rust 结果为 199 个测试通过、2 个显式外部 SDK/独立客户端集成测试 ignored。

本轮证据仅覆盖本地 compiled registry、Models projection、preflight/planning、mock forwarding 和 Provider contract； 未运行真实
OpenRouter/DeepSeek 请求、外部 SDK、Codex/Hermes、负载或长期运行验收。

2026-08-03 在 2026-08-02 快照基础上按 OpenRouter 官方目录精确匹配其中 16 个模型，并修订现有 `ModelConfig` 可表达的描述、
context/input projection、最大输出、输入/输出模态、tokenizer、knowledge cutoff、参数和 reasoning efforts。
`openai/gpt-5.3-codex-spark` 没有精确匹配，未使用相近的
`openai/gpt-5.3-codex` 代替；其 128,000 context、128,000 最大输出和四档 level 为人工修订值。Nemotron canonical
配置采用基础模型上界，不采用 `:free` endpoint 的收窄值；完整采集边界见
[OpenRouter 模型目录快照](../references/openrouter/model-catalog-2026-08-02.md)。

Public Model 现在拥有稳定 `id`、`created`、展示元数据和生命周期。registry 为每个下游 operation 把全部静态可执行 Route
编译为一个内部 `ModelExecutionInterface`；generation 接口同时持有固定 `ModelInterfaceCapabilities` 与有序候选， Embeddings
接口持有分型 `EmbeddingInterfaceCapabilities` 与唯一 Native candidate。generation 布尔能力仅在 全部候选支持时为
`supported`，token 上限只在全部已知时取最小值，模态、参数和 reasoning level 取集合交集；未知保持
`unknown`/`null`。`PublicModelInfo` 只投影该契约的安全副本，用于标准 Models 投影和扩展 Models 响应，不包含 Provider、
Target、Route、upstream model、endpoint、credential、健康或价格信息。preflight 读取同一执行接口；retired 或没有可执行 接口的
Public Model 不进入可见目录。

`text-embedding-3-small` 的固定接口公开四种非空 input form、默认 `float` 和显式 `float`/`base64`、默认维度 1536、单输入
8192 token 与单请求总计 300,000 token；`max_inputs` 由上游 2048 上界和 bootstrap JSON response budget 的 checked
worst-case 序列化边界共同收窄。只有 token-array 两种输入在本地精确计数，字符串 不做 tokenizer 估算。当前没有证据支持精确可变维度域，因此
`dimensions.allowed = null`，显式
`dimensions` 在 egress 前拒绝，接口参数仅包含 `encoding_format` 与 `user`。

Embeddings ingress 只接受 `model`、`input`、`encoding_format`、`dimensions`、`user` 的严格 JSON object， 把 string、string
array、token array 与 token-array array 一次性判别为闭合 union。analysis、preflight 和 planning 从扩展 Models
…5196 tokens truncated…、明确 usage、token
  observation、output speed 和 cache read/write 观测；request/user/credential/ endpoint URL 与正文不进入指标 key。
- `GatewayMetrics::provider_snapshots` 提供进程内只读快照；当前未接入 `/metrics`、Prometheus/OpenTelemetry
  exporter、持久化、分布式聚合或按遥测结果动态重排 Route。
- 新增的 8 个 `observability_contract` 测试覆盖 JSON/streaming usage 与 cache、Provider/route mode 维度、 retry HTTP
  failure、SSE terminal/EOF failure 和下游取消；`cargo test --locked`、`cargo fmt -- --check`、
  `cargo clippy --locked -- -D warnings` 与 `git diff --check` 均通过。该证据仍只覆盖 fake transport 的进程内 采集边界，不证明真实
  Provider 性能、cache 语义、外部 SDK、负载或长期运行结果。

2026-08-03 完成 Chat/Responses definition 命名拆分与标准字段预留：

- 原通用 `EndpointCapabilities` 已拆为可注册的 `ChatCompletionsCapabilities`、`ResponsesCapabilities`，以及只供请求
  分析和公共子集判断使用的 crate-private `GenerationCapabilities`；请求事实字段也使用 `generation` 命名，不再保留 模糊的
  `protocol` capability 公共表面。
- canonical `ModelConfig`/`ModelInfo` 增加可选 `ModelMode`、`InputModality` 和 `OutputModality`；输入枚举覆盖
  text/image/audio/file，输出枚举覆盖 text/image/audio。所有 checked-in Model 均保留 `None`，未知不被解释成空集合。
- Chat 预留 custom tool、audio/file input、audio output、predicted outputs、web search、prompt caching、moderation、 logprobs 与
  multiple choices；Responses 预留 custom/hosted tools、file input、conversation、prompt template、prompt caching、context
  management、标准 `include` 枚举、moderation 与 logprobs。所有 Provider contract 和 Upstream API definition 均保持新增字段为
  `false` 或空集合。
- 没有增加对应 Bridge、adapter 或 Provider 请求实现。Model 的 mode 与输入/输出模态已进入 registry 和扩展模型信息； Upstream
  API definition 若启用任一尚未实现的协议能力，仍触发带稳定说明的 `unimplemented!`。请求分析会逐协议识别 custom/hosted
  tool、audio/file、predicted output、web search、prompt cache、conversation、prompt template、context
  management、include、moderation、logprobs 与 multiple choices 等已预留 wire 语义，并在 route/egress 前返回
  `UnimplementedCapabilities`。Ingress 将其映射为 HTTP 400、code `unimplemented_request`；真正未知的 tool type 继续返回既有
  `UnsupportedCapabilities`。
- `capability_definition_contract` 覆盖 Model mode/模态编译，以及 10 个 Chat definition、10 个 Responses definition、 10 个
  Chat 请求和 10 个 Responses 请求预留触发点；`native_routing_contract` 11 个、`bridge_forwarding_contract` 9 个 和
  `provider_boundary_contract` 16 个测试通过。随后 `cargo fmt -- --check`、`cargo test --locked`、
  `cargo clippy --locked --all-targets -- -D warnings` 与 `git diff --check` 均通过。全量 Rust 结果为 158 个测试通过、 1
  个需要外部 OpenAI Python/Node SDK 的集成测试 ignored；没有修改 protocol corpus，也没有运行外部 SDK、真实
  Provider、负载或长期验证。

2026-08-03 完成固定 Public Model 能力契约与模型信息接口：

- `PublicModelInfo` 现在包含稳定标准身份、生命周期、模型事实，以及 Chat Completions/Responses 各自唯一的
  `ModelInterfaceCapabilities`；同一个 registry 编译对象同时驱动标准四字段投影、扩展 list/detail 和请求预检。
- 固定接口能力由对应协议的全部静态可执行 Route 保守相交。请求只校验客户端明确选择的 Public Model；能力不足或 未知时在
  egress 前返回 HTTP 400 `unsupported_model_capability`，不会改选模型、跳过 Route 或重排 fallback。
- 已接入 `GET /v1/models`、`GET /v1/models/{model}`、`GET /openbridge/v1/models` 和
  `GET /openbridge/v1/models/{model}`。四个接口共享 Bearer 认证和不可变可见目录；未知或 retired 模型统一隐藏， 扩展响应不包含
  Provider、Target、Route、upstream model、endpoint、credential、健康、价格或指标。
- `cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings` 和 `git diff --check` 均通过； 全量
  Rust 结果为 164 个测试通过、1 个需要外部 OpenAI Python/Node SDK 的集成测试 ignored。未修改 `testdata/`
  或 `tools/corpus/`，因此未运行 Python corpus baseline；也未运行外部 SDK、真实 Provider、负载或长期验证。

2026-08-03 完成能力路由遗留审计与需求归并：

- registry compiler 先把每协议的静态可执行候选与 `ModelInterfaceCapabilities` 编译进同一个执行接口；`plan_request`
  在预检后只读取该接口，不再在请求期扫描 Route、解析 Target/API 或判断静态启停。`BridgePlan` 失败会拒绝整个请求， 不会跳过该
  Route；reasoning wire 映射不改变候选资格或顺序。
- `forward_request` 只因 cooldown、credential 可用性、可重试 HTTP/transport failure 和 state affinity 执行
  retry/fallback，不读取或比较模型能力。已删除没有生产调用方的 `ProviderAdapter::validate_capabilities` 及其重复
  `ApiCapabilities` 子集入口；Provider 能力上界仍只在 registry 构建时校验。
- 内部 Public Model 聚合输入已改称 Route contract contribution；请求事实、错误说明和测试名称统一为 Public Model
  固定契约预检，不再把能力描述成候选级运行时检查，也不再使用“兼容候选”术语。
- 新增[Public Model 与模型能力契约](../functional-requirements/model-information-and-capability-contract.md)，集中维护
  身份、生命周期、固定交集、Models API、错误、禁止能力路由、验收与非目标；API、配置、产品范围和 Provider 韧性文档只保留各自所有权并链接该页。
- `cargo fmt -- --check`、`cargo test --locked --target-dir target\model-contract-audit`、
  `cargo clippy --locked --target-dir target\model-contract-audit -- -D warnings` 与 `git diff --check` 均通过；全量 Rust
  结果为 163 个测试通过、1 个外部 OpenAI Python/Node SDK 集成测试 ignored。未修改 `testdata/` 或
  `tools/corpus/`，因此未运行 Python corpus baseline；也未运行外部 SDK、真实 Provider、负载或长期验证。

2026-08-03 完成基于 OpenRouter 精确目录的 Public Model 事实补全：

- `src/models/` 下 16 个可精确匹配模型补全或校验现有描述、模型级 context、输入/输出模态、tokenizer 和 knowledge cutoff，并把
  `top_provider.max_completion_tokens` 映射为最大输出上限。OpenRouter 没有独立的 max-input 字段，因此 `max_input_tokens`
  使用已确认的模型级 `context_length`；不从总上下文减最大输出， 也不为没有精确记录的 `gpt-5.3-codex-spark` 猜测事实。
- registry 会传递并保守相交 tokenizer、knowledge cutoff 和模型描述；不同 Route 的事实不一致或缺失时继续 返回 `null`
  。模型本体模态与实际 Chat/Responses 接口能力分开聚合，目录中的 video 不会扩大当前接口契约。
- `tests/example_config.rs` 覆盖编译目录和加载后的 Public Model 元数据；`tests/forwarding_contract.rs` 覆盖
  `/openbridge/v1/models` 与 detail 的一致性及客户端可见非空字段。`gpt-5.3-codex-spark` 和现有未知字段的
  `null` 语义仍由测试保护。
- 本轮执行 `cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings` 和
  `git diff --check`，均通过；未修改 `testdata/` 或 `tools/corpus/`，因此未运行 Python corpus baseline。 未运行外部 SDK、真实
  Provider、负载或长期验证。

2026-08-03 完成扩展 Models 首版 schema 的 reasoning wire 与参数所有权修正：

- `ReasoningLevel::XHigh` 在扩展 Models list/detail 中统一序列化为请求解析器和 OpenAPI 使用的标准 `xhigh`， 不再返回无法直接回填请求的
  `x_high`。
- canonical `ModelInfo.supported_parameters` 继续参与 Upstream API 收窄和每协议固定接口交集，但
  `ModelCapabilities` 不再重复公开这份模型目录上界；客户端可调用参数只由目标
  `interfaces.*.supported_parameters` 公开。项目尚未发布，因此 schema version 保持首版字符串 `"1"`。
- 聚焦 Models 契约测试先确认旧实现分别因重复参数字段和 `x_high` 失败，最小实现后通过。随后
  `cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings` 与 `git diff --check`
  均通过；全量 Rust 结果为 164 个测试通过、1 个外部 OpenAI Python/Node SDK 集成测试 ignored。
- 本地服务使用私有下游用户 Key 只读验证 7 个扩展模型：schema version 均为 `"1"`，模型事实层均无
  `supported_parameters`，两个接口参数列表均保留，reasoning levels 包含 `xhigh` 且不含 `x_high`，标准/扩展 ID 与所有 detail
  对象一致。该验证未调用真实 Provider；也未运行外部 SDK、负载或长期验收。

2026-08-04 完成同一 Public Model 的多 Provider 聚合装配与 continuation 安全边界：

- `src/providers/catalog/routing.rs` 的单 target registration 已改为有序 Provider route source 列表；对每个 下游协议按
  source 声明顺序生成全部 Native Route，再按同序生成 Bridge Route。已有 checked-in Public Model 的 source 列表仍各只有一个元素，Route
  ID 与默认行为未改变，也未新增未经真实证据确认的 Provider 绑定。
- `src/registry/public_model.rs` 在保守接口交集之外记录仅供内部判断的 Target/API continuation issuer；只有全部 Responses
  Route 支持且 issuer 唯一时才公开 `previous_response_id`。多个潜在签发者会把 typed state 收窄为
  `unsupported`、移出 `supported_parameters`，请求在 transport 前以能力错误拒绝；同一 issuer 的多 Route 仍 保持第一候选执行和禁止
  fallback。
- TDD 首先确认目录测试因 registration 无法表达 source 列表而编译失败，并确认歧义 continuation 旧实现实际 调用首选
  Target、返回 429 而不是预期的本地 400。修复后，目录顺序、跨 Provider 同 canonical Model 规划、 能力交集、真实 attempt 顺序、唯一
  issuer continuation 与歧义 preflight 聚焦测试均通过。
- 最终执行 `cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings` 与
  `git diff --check`，全量 Rust 结果为 166 个测试通过、1 个需要外部 OpenAI Python/Node SDK 的集成测试 ignored，Clippy
  零告警。未修改 `testdata/` 或 `tools/corpus/`，因此未运行 Python corpus baseline；未运行 外部 SDK、真实 Provider、负载或长期验收。

2026-08-04 完成 Route planning 职责拆分与 reasoning wire 映射下沉：

- `src/pipeline/preflight.rs` 独立负责 Public Model/协议解析、固定接口能力与限制预检；`planning.rs` 在预检后 消费已编译的候选，构造
  Native/Bridged candidate 并设置 fallback 边界。
- `RouteCandidate` 不再保存已应用的 reasoning mapping，Native 与 Bridged `ApiRequest` 在 RoutePlan 中均保留目标 协议的
  canonical level。OpenAI-compatible Provider adapter 在 egress JSON 准备阶段与真实 model 一次性完成 target-specific
  reasoning wire 改写；`PreparedUpstreamRequest` 只携带实际已应用映射用于 attempt tracing。
- TDD 首先让规划测试因旧实现提前把 `xhigh` 改成 `max` 而失败；重构后该测试确认所有规划候选保留 `xhigh`， forwarding
  contract 继续确认发送到显式映射 Upstream API 的 wire 值为 `max`。`cargo fmt -- --check` 与
  `cargo test --locked` 均通过；全量 Rust 结果为 166 个测试通过、1 个外部 OpenAI Python/Node SDK 集成测试 ignored。未修改
  `testdata/` 或 `tools/corpus/`，未运行 Python corpus、外部 SDK、真实 Provider、负载或长期验收。

2026-08-04 完成 Public Model 协议执行接口预编译：

- registry compiler 在完成引用、operation 和 Native/Bridged 方向校验后，按 Public Model/下游 operation 一次性编译
  `ModelExecutionInterface`。每个接口把保守的 `ModelInterfaceCapabilities` 与同一组静态启用候选绑定；候选冻结
  Route/Target/typed upstream operation、上下游协议、模式、Bridge 所需的 upstream model 与 reasoning output。扩展 Models DTO
  仍只投影安全能力，不暴露上述拓扑。
- `preflight` 和 `planning` 现在读取同一个执行接口：前者完成一次能力校验，后者仅按预编译顺序构造 Native 请求或
  `BridgePlan`。请求路径不再扫描 `PublicModel.routes()`、查询全局 Route 表或重复判断 Target/API 静态启停；forwarding 仍独占
  credential member、retry/fallback、cooldown、取消和 state-affinity 行为。
- TDD 先加入 compiler 单元测试，旧代码因缺少 `execution_interface` 编译失败；实现后该测试验证禁用 Chat Native API 时只保留
  Chat→Responses Bridge。`native_routing_contract` 进一步验证禁用的弱 Route 既不收窄公开能力，也不进入 RoutePlan；既有多
  Provider Native-first、continuation issuer 和 forwarding 回归保持通过。
- 已通过 `cargo test --locked --test example_config`、`cargo test --locked --test native_routing_contract`、
  `cargo test --locked --test forwarding_contract`、`cargo fmt -- --check`、`cargo test --locked`、
  `cargo clippy --locked -- -D warnings` 和 `git diff --check`。未修改 `testdata/` 或 `tools/corpus/`，因此未运行 Python
  corpus baseline；未运行外部 SDK、真实 Provider、负载或长期验收，SDK 兼容测试仍为 ignored。

2026-08-05 完成 Native Embeddings 垂直链路：

- `OperationKind::EmbeddingsCreate`、分型 Embedding capability/execution interface、严格 request union、固定 Native
  planning/adapter、预提交有界成功体 validator、有限 replay/取消、稳定错误 envelope 和 `operation` 级脱敏观测已接入 生产
  Router；Models projection 与 preflight 使用同一个预编译接口。
- checked-in registry 新增 `openai/text-embedding-3-small`、专用 `openai-text-embedding-3-small` target、`embeddings`
  Upstream API、`embedding-primary-openai-embeddings` Route 与 Public Model `embedding-primary`。OpenAPI、配置说明、功能
  需求、实现细节和 README 已同步到该首版固定契约。
- 12 个聚焦 Rust target 共 137 个测试通过；独立的 Python 3.12.9 标准库客户端在 Windows loopback 上先读取
  `/openbridge/v1/models`，再调用 `/v1/embeddings`，1 个显式 ignored integration case 通过。该闭环使用实际 Router、
  checked-in registry 和 deterministic in-memory upstream，不安装第三方包、不读取私有配置，也不调用真实 Provider。
- 最终执行 `cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings` 与
  `git diff --check`，均通过；全量 Rust 结果为 198 个测试通过、2 个显式 integration test ignored，其中独立 Python loopback
  已按上条单独运行通过；Clippy 零告警。未修改 `testdata/` 或 `tools/corpus/`，因此未运行 Python corpus baseline；未运行外部
  OpenAI SDK、真实 Provider、负载或长期验收。

### 2026-08-05 移除本机 Codex probe 链

- 删除 `src/codex_auth.rs`、`src/codex_identity.rs` 和独立 auth-file contract；生产代码不再读取本机 Codex auth state、
  terminal 环境或 OS/architecture identity，也不构造 Codex CLI-compatible User-Agent；
- `openbridge-probe` 删除 ChatGPT 专用 credential、selector、report、model-list query、streaming Responses payload/SSE session
  和对应 fixture，只保留已启用 target + 配置绑定 API-key pool 的通用 probe；
- probe 现在对所有禁用 target 使用同一错误，并在 credential 读取和 egress 前失败；默认禁用的 ChatGPT target 因此没有可执行
  probe 或数据面入口；
- ChatGPT Provider/Provider instance/target、OAuth2 contract 与 OpenBridge-owned `OAuth2CredentialManager` 保留；PKCE、refresh、
  manager 到 Provider adapter 的受控借用和真实 Provider 验收仍需独立焦点；
- 移除仅由本机 identity 构造使用的 `os_info` 直接依赖及对应 lockfile 依赖树；
- TDD 先证明 CLI 仍接受旧 auth-file selector、禁用 target 仍返回 Codex identity 专用错误；实现后 CLI 2 项、probe 9 项、Provider 6 项、
  OAuth config 9 项和 startup 3 项聚焦测试均通过；
- `cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings` 与 `git diff --check` 均通过；全量 Rust
  结果为 239 个测试通过、2 个显式外部客户端 integration test ignored。未修改 `testdata/` 或 `tools/corpus/`，因此未运行 Python
  corpus baseline；未运行真实 Provider、外部 SDK、Agent runtime、负载或长期验收。

## 当前未实现

当前 checked-in 五个数据面 Provider 注册项没有在缺少真实能力证据时预设 reasoning level 映射；功能只在具体 Upstream API
显式声明后生效。Bridged Route 支持明文 reasoning channel 的受限转换，但不支持 opaque
`encrypted_content` continuation 或把 summary/content 伪造成 user-visible text。

- OpenRouter 有状态 Responses、真实异构协议 Provider、可配置 ConversionPolicy 和 Bridge continuation ledger；
- Responses WebSocket、Realtime、Files、Conversations 等资源 API；
- ChatGPT PKCE/device login、refresh token 读取/轮换、续约调度、401 recovery、持久化、多账号 pool，以及 ChatGPT
  Route/Public Model 数据面；
- keyring、加密 secret 文件、远程 secret manager 和动态 credential 控制面；
- 动态 health/weight、持久化或分布式 cooldown 与后台探测；
- OpenTelemetry/Prometheus exporter、指标 HTTP API、持久化或分布式聚合；
- 多 Embeddings candidate、embedding Bridge、string tokenizer、可变维度域，以及向量转换、缓存、索引或检索；
- hosted tool、MCP Tool Bridge 或非 loopback 部署。

## 相关资源

- [当前代码架构](current-architecture.md)
- [Public Model 与模型能力契约](../functional-requirements/model-information-and-capability-contract.md)
- [能力探测](capability-probing.md)
- [协议测试语料与工具](protocol-test-corpus.md)
- [配置、凭证与受信边界](../functional-requirements/configuration-and-credentials.md)
- [路由与 Provider 韧性](../functional-requirements/provider-resilience.md)
