# 当前实现说明

本文是当前 checkout 的实现状态快照，只记录已经存在于代码、确定性测试或明确本地验证中的最终内容。
本文不保留按日期排列的变更历史、修复前后对比或后续设计过程；新的实现完成后直接更新对应主题。
模块职责和内部数据流见[当前代码架构](current-architecture.md)，详细遥测口径见[遥测指标](telemetry-metrics.md)。

OpenBridge 仍是未发布的实验性原型。确定性测试、loopback/独立客户端验证、真实 Provider、外部 SDK、负载和长期运行分别代表不同证据层，
任何一层都不能替代其他层。

## 运行入口与配置

使用不含真实凭证的模板创建私有配置后启动服务：

```bash
cp config/users.example.toml config/users.toml
cp config/upstream-credentials.example.toml config/upstream-credentials.toml
# 编辑两份私有 TOML；填写用户/API key，并按需配置 ChatGPT auth_json_file。
cargo run --bin openbridge --locked
```

`config/bootstrap.toml` 使用 schema v2，当前只接受 loopback listener，并配置用户文件、上游 credential 文件、请求/响应/replay/SSE
资源上限和共享 HTTP client 限制。Provider family、Provider instance、Model、Upstream Target、Upstream API、Route、Public Model
和 credential pool binding 全部由 Rust 代码注册；Provider instance 唯一拥有受信 BaseURL。上述配置变更需要重启或重新编译。

Trace export 默认禁用。显式配置 `[telemetry.traces].otlp_http_endpoint` 后，配置所有者可以选择 loopback、非 loopback IP 或 DNS
host，但 URL 必须是无 userinfo、无自定义 path、query 或 fragment 的绝对 HTTP base；OpenBridge 固定发送到 `/v1/traces`，不接受
exporter header、认证信息或请求级 endpoint 覆盖。collector 的真实远程网络、TLS 和长期运行能力不在当前验证范围内。

| 私有运行配置 | 模板 |
|---|---|
| `config/users.toml` | `config/users.example.toml` |
| `config/upstream-credentials.toml` | `config/upstream-credentials.example.toml` |
| `config/bootstrap.toml` | `config/bootstrap.example.toml` |

下游用户来自私有 `users.toml`，上游 credential binding 在有序 `api_keys` 与单一 `auth_json_file` 中二选一。服务在监听前校验配置，
建立不可变 `UserRegistry`、API-key `CredentialStore` 和 OAuth2 credential snapshot；未知、重复、类型不匹配、损坏或不完整的 binding
会阻止启动。用户配置、API-key Store 和普通 TOML 不热重载；OAuth2 manager 只在显式登录或到期 refresh transaction 中 guarded reload、
原子写回并发布新 generation。

ChatGPT Provider 仅作为默认禁用的独立 OAuth2 target 注册，不加入 Route 或 Public Model。`openbridge-auth login chatgpt` 使用固定的
device interaction 与 PKCE 流程事务性写入 OpenBridge-owned auth 文件；服务不读取本机 Codex auth state、环境变量、terminal identity
或 Codex probe。已配置的 OAuth2 bundle 支持 expiry-driven refresh，但当前没有 ChatGPT 数据面 credential 借用或 401 recovery。

## 当前 HTTP 接口

除 `/healthz`、OpenAPI 和 Swagger UI 外，表中接口均使用启动时用户表中的静态 Bearer API key：

| Endpoint | 当前行为 |
|---|---|
| `GET /healthz` | 返回最小存活状态和 `registry_version`，不访问上游。 |
| `GET /openapi.yaml` | 返回当前构建内置的 OpenAPI 3.0.3 YAML。 |
| `GET /swagger-ui`、`GET /swagger-ui/` | 返回本地接口验证页面。 |
| `GET /v1/models`、`GET /v1/models/{model}` | 返回 Public Model 的 OpenAI 标准四字段 list/retrieve。 |
| `GET /openbridge/v1/models`、`GET /openbridge/v1/models/{model}` | 返回模型事实和每个 operation 的固定能力契约。 |
| `GET /openbridge/v1/metrics` | 返回本次进程运行期间的进程级指标快照。 |
| `GET /openbridge/v1/metrics/providers` | 返回按 Provider attempt 低基数维度聚合的指标快照。 |
| `POST /v1/chat/completions` | 执行 Chat Native 或显式 Chat→Responses Bridge 的 JSON/SSE。 |
| `POST /v1/responses` | 执行 Responses Native 或显式 Responses→Chat Bridge 的 JSON/SSE。 |
| `POST /v1/embeddings` | 执行独立 Embeddings Native Route，并在下游提交前校验有界 JSON 成功体。 |

OpenAPI 源文件是 [`docs/openapi.yaml`](../openapi.yaml)，Swagger UI 源文件是 [`docs/swagger-ui.html`](../swagger-ui.html)。标准和扩展
Models 接口共享同一不可变可见目录；响应不暴露 Provider、Target、Route、upstream model、endpoint、credential、健康或价格信息。

## Provider、Model 与 Route

当前注册六个 Provider family：OpenAI、LongCat、OpenRouter、DeepSeek、Xiaomi MiMo 和 ChatGPT。可调用 Public Model 与固定
Upstream Target 如下：

| Provider / credential pool | Public Model | Upstream Target | 当前 Route surface |
|---|---|---|---|
| OpenAI / `openai-primary` | `gpt-5.6-sol` | `openai-main` | Chat/Responses Native-first，并有反向 Bridge 候选 |
| OpenAI / `openai-primary` | `text-embedding-3-small` | `openai-text-embedding-3-small` | 唯一 Embeddings Native Route；无 Bridge 和 fallback |
| LongCat / `longcat-primary` | `LongCat-2.0` | `longcat-2` | Chat/Responses Native-first，并有反向 Bridge 候选 |
| DeepSeek / `deepseek-primary` | `deepseek-v4-pro` | `deepseek-v4-pro` | Chat Native |
| DeepSeek / `deepseek-primary`；OpenRouter / `openrouter-primary` | `deepseek-v4-flash` | `deepseek-v4-flash`；`openrouter-deepseek-v4-flash` | DeepSeek Chat Native；OpenRouter Chat/Responses Native |
| Xiaomi MiMo / `mimo-primary` | `mimo-v2.5-pro`、`mimo-v2.5` | 对应 MiMo target | Chat/Responses Native-first，并有反向 Bridge 候选 |
| ChatGPT / `chatgpt-codex` | 无 | 默认禁用的 ChatGPT target | 无 Route/Public Model；通用 probe 拒绝禁用 target |

Canonical Model 事实与 Public Model 身份分离。目录中存在未绑定 Target 或 Route 的模型 profile 时，不代表它可被客户端调用。
同一 Public Model 的 Provider source 由代码显式列出；编译顺序为每个协议先生成全部 Native 候选，再生成同顺序的 Bridge 候选。
当前 `deepseek-v4-flash` 是已注册的多 Provider 聚合，先尝试 DeepSeek，再按配置顺序使用 OpenRouter；不会按 canonical model ID
自动发现或隐式聚合其他 Provider。

每个 Public Model 的 operation 接口由 registry 在启动时预编译。固定能力是所有可执行候选的保守交集：未知事实保持 `unknown`/`null`，
能力不足在 egress 前以稳定的本地错误拒绝；请求不会因能力不足改选模型、跳过前序 Route 或重排候选。Models projection 和 request
preflight 读取同一执行接口，执行拓扑不向下游公开。

## 已实现的请求行为

### Generation

- `POST /v1/chat/completions` 和 `POST /v1/responses` 支持已声明范围内的 JSON 与 SSE；客户端只选择 Public Model，不选择 Provider、真实模型、
  URL、credential 或 Route。
- Native Route 保留目标协议的 canonical request，由 Provider adapter 在 egress 阶段写入实际 upstream model，并按已声明规则执行 reasoning
  level wire mapping；未声明或未知 level 在 egress 前拒绝。
- `Bridged` Route 只转换明确 allowlist 内的 text、function tool、tool result、明文 reasoning channel、非流式 JSON 和流式 SSE。
  tool-call identity、fragmented arguments、响应/项目索引和终态由两种协议各自的 stream state machine 维护。
- Bridge 不猜测或伪造 opaque continuation、encrypted reasoning、image/file/audio、structured output、hosted/custom tool、后台状态或其他
  Provider 私有扩展；无法表达的语义在上游调用前拒绝。当前没有通用异构 Provider 或可配置 ConversionPolicy。
- Responses 的 `previous_response_id` 只有在可执行 Responses Route 全部绑定同一且唯一的 issuing Target/API 时才公开；存在多个潜在签发者时，
  请求在 transport 前拒绝，不能盲投首选 Provider。无状态请求不保存或迁移上游 response state。

### Embeddings

`text-embedding-3-small` 使用独立 Public Model、OpenAI target 和唯一 Native Route。请求只接受严格 JSON union：string、string array、token
array 或 token-array array；公开 `encoding_format`、`user` 以及默认 1536 维度。当前没有经证实的可变 dimensions 域，显式 `dimensions` 会在
egress 前拒绝。单输入、单请求和 JSON response 都受启动配置及上游限制共同约束；字符串不做 tokenizer 估算，token array 只做本地精确计数。

成功体在下游 response commit 前一次性完成有界 JSON 校验；usage 只记录明确返回的字段，非法成功体不进入 retry。当前 Embeddings 没有
Bridge、多候选、向量转换、缓存、索引、检索或独立 tokenizer。

### Retry、fallback、取消与安全出站

- 请求级和 candidate 级 retry 有固定硬预算与 capped backoff；可重试的 HTTP/transport failure 只沿同一 Public Model 的已注册 Route 顺序
  fallback，首个下游业务输出提交后不再切换上游。
- 429 支持按 credential pool 的有序 member 轮换；单进程共享 member cooldown 和 target fault cooldown，不持久化、不跨进程、不动态探测。
- 下游取消会传播到当前 send、backoff、response body 和后续 attempt；SSE framing、terminal、EOF-before-terminal 和 body error 均在对应边界
  收口一次。
- 上游 endpoint、credential、Authorization、proxy header 和 transformation rule 只能来自受信代码注册与 purpose-bound credential boundary；
  业务请求不能覆盖这些值。普通固定 header 和 Provider hook 仍拒绝敏感 header 名称。

## 观测、Probe 与隐私边界

每个已认证请求在 response body 正常 EOF、流错误或下游取消时结束一次 `downstream_request` 生命周期；每次实际上游 attempt 有独立的
`provider_attempt` 生命周期。进程内提供无持久化、低基数的 gateway 和 Provider attempt 快照，并通过上述两个 Bearer endpoint 读取；读取
接口本身不创建 observation，也不稀释计数分母。

显式启用 OTLP/HTTP trace 后，只导出 reviewed 的 `downstream_request` root 和 `provider_attempt` child，使用固定 protobuf、空 exporter
headers、有界 queue/batch/export/shutdown timeout。collector failure、timeout 或 backpressure 只丢弃 telemetry，不影响业务响应、retry、
fallback 或 Provider 结果。OTLP metrics、OTLP logs、Prometheus exporter、持久化、历史查询、重置和分布式聚合尚未实现。

trace、日志、指标和错误不包含 Authorization、credential、用户身份、真实 endpoint URL、raw path/query、请求/响应正文、tool/reasoning
内容或原始错误正文；指标 key 只使用已校验的 Provider、route、target、typed upstream operation、Public Model、downstream operation、
route mode 和 streaming 等低基数维度。详细字段口径见[遥测指标](telemetry-metrics.md)。

`openbridge-probe --target <id>` 只探测管理员显式选择的、已启用的固定 Target，复用该 Target 的 adapter 与 transport；它不接受 URL、model、
header 或 credential 覆盖，不修改 registry，不加载下游用户 key，也不读取本机 Codex 状态。

## 当前未实现与明确边界

- ChatGPT credential manager 到数据面的借用、Route/Public Model、真实请求、401 recovery、多账号 pool 和账号级负载均衡；
- OpenRouter 有状态 Responses、Responses WebSocket、Realtime、Files、Images、Videos、Conversations 等资源或实时协议；
- 真实异构协议 Provider、通用动态 Provider/plugin DSL、动态 Converter catalog、可配置 Bridge policy 和 continuation ledger；
- Native 多模态扩展的完整已验证交付，以及 image/file/audio、hosted tool、MCP Tool Bridge 的跨协议转换；
- keyring、加密 secret 文件、远程 secret manager、动态 credential 控制面；
- 动态 health/weight、持久化或分布式 cooldown、后台探测、多进程协调和非 loopback 服务部署；
- OTLP metrics、OTLP logs、Prometheus exporter、指标持久化、历史查询、重置、外部分析协议和分布式聚合；
- 多 Embeddings candidate、embedding Bridge、string tokenizer、可变维度域、向量转换/缓存/索引/检索；
- 由网关执行 function tool、GUI、Web 控制台、在线用户管理、配额、计费、审计或独立控制面。

## 验证状态

当前实现已有以下确定性证据：

- Rust contract/integration tests 覆盖配置与 credential 边界、compiled registry、Models projection、Native/Bridge planning、HTTP/SSE、retry/fallback、
  cooldown、取消、Embeddings 有界 JSON、OAuth2 lifecycle、metrics 和 OTLP trace filtering；
- 本次状态整理实际运行并通过 `cargo test --locked -j 1 --test config_contract`（15 项）和
  `cargo test --locked -j 1 --test otlp_trace_contract`（2 项）；
- 已记录的 Rust 基线中，`cargo test --locked -j 1` 和 `cargo clippy --locked -- -D warnings` 通过；变更 Rust 文件的
  `rustfmt --check --edition 2024` 与 `git diff --check` 通过；
- 当前仓库级 `cargo fmt -- --check` 仍受未修改的 `src/transport/mod.rs` module 声明顺序差异影响，不能把该命令描述为通过；
- OTLP contract 使用 loopback fake collector 验证 protobuf、父子 span、脱敏、禁用态零 egress、collector failure isolation 和有界 shutdown；非 loopback
  IP/DNS 当前只验证启动配置解析；
- 本次目标仅为文档整理，未修改 `testdata/` 或 `tools/corpus/`，因此未运行 Python corpus baseline。

以下验收层仍未运行或不由上述结果覆盖：真实 Provider 请求、官方 OpenTelemetry Collector smoke、外部 OpenAI SDK、Codex/Hermes 等 Agent runtime、负载、
吞吐、分位数、SLA 和长期运行。确定性测试和单次本地调用不构成这些验收结论。

## 相关资源

- [当前代码架构](current-architecture.md)
- [遥测指标](telemetry-metrics.md)
- [Public Model 与模型能力契约](../functional-requirements/model-information-and-capability-contract.md)
- [Embeddings 与 Native 多模态扩展需求](../functional-requirements/embedding-and-native-multimodal.md)
- [配置、凭证与受信边界](../functional-requirements/configuration-and-credentials.md)
- [路由与 Provider 韧性](../functional-requirements/provider-resilience.md)
- [能力探测](capability-probing.md)
- [协议测试语料与工具](protocol-test-corpus.md)
- [当前开发焦点](../implementation-plans/current-focus.md)
