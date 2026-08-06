# OpenBridge 项目说明

## 项目定位

OpenBridge 的核心是一个 **单配置所有者、单服务、headless 的多 Provider Agent API 聚合网关**：部署在本地或所有者控制的环境中，通过显式
Rust 代码注册表管理上游 Provider、模型与路由，通过启动时加载的私有用户表认证下游 API Key，并向本地 Agent 或 SDK 提供稳定的
OpenAI-compatible 接口。它不提供 GUI、Web 控制台或在线用户管理功能。

当前处于 **设计探索与原型验证阶段**。仓库中的 Rust 代码用于验证 HTTP/SSE、路由快照、能力检查和 fallback
等关键假设，不代表最终模块边界、Provider 抽象或协议桥接方案已经收敛。开发采用 TDD：每次只选择一个可观察行为，先写会失败的测试，再以最小实现使其通过。

核心方向：

1. 原生转发 `POST /v1/responses`、`POST /v1/chat/completions` 的 HTTP JSON/SSE，以及独立的 `POST /v1/embeddings` JSON；
2. 聚合多个 Provider instance、Upstream Target 与稳定 Public Model；
3. 以每 Provider family 独立 Rust 模块承载协议行为，以显式注册表管理 Provider instance、Model、Upstream Target、Upstream API 和 Route；
4. 在原生协议不可用时，对明确支持的语义执行 Chat ↔ Responses bridge；
5. 正确处理 SSE、tool-call identity、continuation state、取消、有限 retry、target cooldown、首输出前 fallback 与最终错误传播；
6. 优先用 OpenAI SDK、独立 Python 脚本或 curl 验证客户端可见 HTTP/SSE；Codex、Hermes 等 Agent runtime 只在明确宣称对应兼容时验证。
7. 以 bootstrap-only 配置管理进程资源策略，以私有 TOML 管理上下游 credential，并通过 headless 输出提供调用量、usage、TTFT/TTFB
   和终态错误率统计。

现阶段已批准的扩展目标只包括：

- 已完成当前确定性实现与 loopback 验证的 OpenAI-compatible `POST /v1/embeddings`；
- Chat/Responses 同协议 Native image、inline/URL file 与 Chat input audio；
- ChatGPT subscription OAuth：保留默认禁用的独立 Provider，使用 OpenBridge-owned OAuth2 文件完成显式 PKCE 登录与到期驱动
  token 续约；不导入本机 Codex 状态，数据面接入仍须另立焦点。

所有目标必须分别进入独立的当前焦点并串行实施。Embeddings、OAuth2 启动快照、ChatGPT 登录与 refresh 已完成；ChatGPT 数据面接入和
Native 多模态不能并行展开。当前代码事实以[实施现状](docs/implementation-status/current-implementation.md)为准。Images、Files、专用 Audio、Videos
与 Realtime 只保留协议参考，不在现阶段实施范围。

核心稳定后再考虑：

- Provider-hosted tool facade；
- Anthropic Messages 协议兼容与异构 Provider 验证（与 Provider-hosted tool facade 同级）；
- 本地/MCP Tool Bridge；
- headless 的健康、日志与诊断；
- 更多路由策略。

## 当前可运行基线

当前 checkout 已实现 OpenAI `gpt-5.6-sol`、LongCat 与 Xiaomi MiMo 的 Chat/Responses HTTP JSON/SSE 原生转发， OpenRouter 的
`deepseek-v4-flash` Chat 与无状态 Responses Native 路由，以及 DeepSeek V4 的 Chat Native 路由， 并通过独立 Public Model
`text-embedding-3-small` 把 Embeddings JSON 请求固定转发到专用 OpenAI target
`openai-text-embedding-3-small`，其 upstream model 为 `text-embedding-3-small`。该 Embeddings 链路使用严格请求
union、预提交有界成功体校验、单 Route 有限 retry 与 operation 级脱敏观测；显式 `dimensions` 暂不公开，默认维度为 1536。
同时实现有序 Route、固定且不参与 Route 选择的 Public Model capability gate、标准/扩展 Models 接口、输出前
retry/fallback、HTTP 429 credential rotation、单进程 member/fault cooldown、SSE framing 校验和下游断开时的上游 stream
取消传播。显式 `Bridged` Route 还可在两协议间转换 已声明可转换的 text、明文 reasoning channel、function tool、tool
result、非流式 JSON 与流式 SSE；Bridge 对未知字段、未确认的 reasoning 输出、opaque continuation、hosted/custom
tool、image、structured output 和后台状态会在 egress 前拒绝。Native Route 则按选定 Provider/Upstream API
保留固定公共契约已接受的原生语义和同协议合法字段；单条 Native Route 的额外能力不会 扩大 Public Model。当前编译注册项仍优先使用各
Provider 自身的 Native API，尚未注册真实异构协议 Provider。 每个已认证请求在 response body 正常 EOF、流错误或
下游取消时结束一次观测，并提供脱敏 tracing 事件与进程内低基数累计值。

仓库内的 [`config/bootstrap.toml`](config/bootstrap.toml) 只配置监听和资源限制；Model 位于 [`src/models`](src/models)
，Provider adapter、Provider instance 与 Upstream Target/Upstream API 位于 [`src/providers`](src/providers)，Route 与 Public Model
由顶层代码注册表显式组合。每个运行配置都有不含真实凭证的 `.example` 模板：

| 运行配置                           | 模板                                       |
|------------------------------------|--------------------------------------------|
| `config/bootstrap.toml`            | `config/bootstrap.example.toml`            |
| `config/users.toml`                | `config/users.example.toml`                |
| `config/upstream-credentials.toml` | `config/upstream-credentials.example.toml` |

```bash
cp config/users.example.toml config/users.toml
cp config/upstream-credentials.example.toml config/upstream-credentials.toml
# 编辑两份私有 TOML；填写用户/API key，并按需启用注释中的 ChatGPT auth_json_file。
cargo run --bin openbridge --locked
```

`config/users.toml` 与 `config/upstream-credentials.toml` 已被 Git 忽略；仓库只提交不含真实凭证的示例文件。 服务与
`openbridge-probe` 不从进程环境变量或 `.env` 读取上游 API key。用户、API Key、OAuth2 locator、Provider、Model 和 Route 只在启动时
加载；OpenBridge-owned OAuth2 bundle 进入独立 lifecycle manager，并按 access expiry 在文件锁内 guarded reload、refresh 和原子写回。
TOML、用户与 API-key Store 不热重载。请求观测不保存业务正文或 credential；request/user/credential/endpoint URL 不进入指标 key；Provider attempt 遥测
与 trace 只使用已校验的 Provider family、route、target、typed upstream operation 和 Public Model 身份作为低基数维度。

配置 ChatGPT `auth_json_file` 后，可在启动服务前显式登录：

```bash
cargo run --locked --bin openbridge-auth -- login chatgpt
```

该命令显示固定 verification URI 和一次性 device code，完成 authorization-code + PKCE exchange 后才事务性写入完整 bundle；它不接受
issuer、client、endpoint、header、auth-file 或其他应用 cache override。常驻服务只做 expiry-driven 自动 refresh，不会隐式启动交互式登录。

默认监听 `127.0.0.1:8080`。健康检查：

```bash
curl -i http://127.0.0.1:8080/healthz
```

当前运行指标（仅内存快照，需有效下游 Bearer token）：

```bash
curl http://127.0.0.1:8080/openbridge/v1/metrics \
  -H 'Authorization: Bearer replace-with-a-local-client-token'

curl http://127.0.0.1:8080/openbridge/v1/metrics/providers \
  -H 'Authorization: Bearer replace-with-a-local-client-token'
```

两个读取请求只做认证和快照序列化，不计入快照自身，因此连续抓取不会稀释请求错误率分母。

本地接口测试页与机器可读规范：

```text
Swagger UI:  http://127.0.0.1:8080/swagger-ui/
OpenAPI:    http://127.0.0.1:8080/openapi.yaml
```

Swagger UI 是用于本地接口验证的静态页面；点击 `Authorize` 填入下游 Bearer API key 后，即可在页面内测试受保护的标准/扩展
Models、当前运行指标、`/v1/chat/completions`、`/v1/responses` 和 `/v1/embeddings`。页面依赖固定版本的 jsDelivr Swagger UI 静态资源，
OpenAPI 规范由本地服务提供。

原生请求示例：

```bash
curl http://127.0.0.1:8080/v1/chat/completions \
  -H 'Authorization: Bearer replace-with-a-local-client-token' \
  -H 'Content-Type: application/json' \
  -d '{"model":"gpt-5.6-sol","messages":[{"role":"user","content":"hello"}]}'
```

Embeddings 客户端应先读取扩展 Models 中的固定接口，再只发送该接口公开的参数：

```bash
curl http://127.0.0.1:8080/openbridge/v1/models/text-embedding-3-small \
  -H 'Authorization: Bearer replace-with-a-local-client-token'

curl http://127.0.0.1:8080/v1/embeddings \
  -H 'Authorization: Bearer replace-with-a-local-client-token' \
  -H 'Content-Type: application/json' \
  -d '{"model":"text-embedding-3-small","input":["alpha","beta"],"encoding_format":"float"}'
```

请求先按所选 Public Model 的唯一接口契约完成一次能力预检；通过后保持全部配置 Route 的原顺序。代码目录允许 一个 generation
Public Model 显式列出多个 Provider route source；对每个 generation 下游协议，先按 Provider 声明顺序生成全部 Native
候选，再按相同顺序生成 Bridge 候选。相同 canonical Model ID 不会触发自动发现或隐式聚合。当前 checked-in generation Public
Model 的 source 列表中，`deepseek-v4-flash` 显式绑定 DeepSeek 与 OpenRouter 两个 Provider；其余已接入模型目前各自只有一个
source。Native Route 规划保留 canonical 请求；Provider adapter 在准备选定 Upstream API 的 egress 请求时写入实际上游
`model`， 并可对 canonical Model 已声明的 reasoning level 应用显式 wire 映射（例如 `xhigh → max`），其余 JSON 与上游
JSON/SSE body 原生转发。 没有映射的已支持 level 保持原值，未知下游 level 继续在 egress 前拒绝；后续 Route 的额外能力不能扩大
Public Model 契约或导致跳过前序 Route。
`Bridged` Route 则先生成受限 `BridgePlan`，只转换显式 allowlist 内的共同语义并渲染目标协议 wire。 聚合 Responses Route
只有在全部候选支持 continuation 且唯一 Upstream Target/API 可确定时才公开
`previous_response_id`；多个潜在签发者没有 issuer ledger，必须在任何上游调用前拒绝，不能盲投首选 Provider。Provider definition 可声明
固定的非敏感 `User-Agent`/普通 header，受信 request-header hook 也可按编译期规则增添、替换、转换或删除普通 header；请求组装顺序为
hook、固定 header、purpose-bound authentication。OpenAI 与 LongCat 转发 `User-Agent`，OpenRouter 不转发可选 attribution/routing
header；ChatGPT 使用固定 Codex CLI `0.146.0` headless Linux x86_64 兼容 UA 与 `originator`，但其 target 仍默认禁用且没有数据面
Route。共享层不维护
普通 header allowlist，客户端不能指定上游 URL、credential、认证 header、固定 header 或转换规则。Transient upstream failure 在提交下游
response 前使用请求级硬预算与 capped exponential backoff；候选局部重试耗尽后只沿同一 Public Model 已配置的完整 Route fallback，下游断开会
取消当前 send、退避和后续 attempt。

OpenRouter 当前注册固定 target `openrouter-deepseek-v4-flash`，使用 `openrouter-primary` credential pool，把 Public Model
`deepseek-v4-flash` 原生转发到基础模型 `deepseek/deepseek-v4-flash`。该注册项支持 Chat Completions 和无状态 Responses；
`store: true`、非空 `previous_response_id` 与 `background: true` 会在 egress 前拒绝。 它不启用 Protocol Bridge、fallback
或带额外会话记录政策的 `:free` 变体。

DeepSeek 当前注册 `deepseek-v4-pro` 与 `deepseek-v4-flash`，共享 `deepseek-primary` pool 和固定
`https://api.deepseek.com` endpoint。两个 DeepSeek target 都只注册 Chat Native API；`deepseek-v4-pro` 对下游仅公开 Chat，
`deepseek-v4-flash` 通过显式 OpenRouter source 增加无状态 Responses Native。Chat 的 reasoning 输出能力明确配置为
`PlainText`（对应 `reasoning_content`），不把它伪装成 DeepSeek 原生 Responses 能力。LongCat 当前 Chat/Responses 均配置为
`Unknown` reasoning 输出；现有协议、文本和工具测试没有证明 可读 reasoning，因此只有 Native 路径可保留这类上游语义，Bridge
不会猜测转换。Xiaomi MiMo 当前注册
`mimo-v2.5-pro` 与 `mimo-v2.5`，使用
`mimo-primary` pool 和固定 `https://api.xiaomimimo.com` endpoint；两个模型都提供 Chat/Responses Native-first Route 及同
target 的反向 Bridge 候选。MiMo 两个协议的 reasoning 输出能力均为 `Unknown`，尚未增加可读 reasoning wire 映射；两个 Native
API 声明支持 image input、structured output 和 `parallel_tool_calls`，但仍关闭 `store`、`background` 与
`previous_response_id`。image 与 structured output 只是 Native API 事实；反向 Bridge 不支持时，它们不会进入 Public Model
的固定接口契约。`parallel_tool_calls` 只有在全部对应 Route 都支持时才对下游公开。

下游用户和 API Key 来自私有 `users.toml`；私有 `upstream-credentials.toml` 的每个编译期 binding 在有序 `api_keys` 与单一
`auth_json_file` 中二选一。代码注册表只保存非敏感的 pool id、Provider 和 credential kind，不保存 secret locator。服务在监听前
把已启用的上下游 Key 合并为不可变 `CredentialStore`，并把显式配置的 ChatGPT OAuth2 bundle 装入独立的
`OAuth2CredentialManager`。未知、缺失或重复 binding、source/kind 错配、无效 API-key pool 或损坏/不完整 OAuth2 bundle 都会阻止
启动；完整但已过期的 bundle 会保留并在 worker 启动后立即 refresh。进程环境变量和 `.env` 不再是上游 key 来源；运行时不重新读取两份
TOML，但 OAuth manager 会在每次到期 refresh 前锁定并 reload 自有 auth 文件，成功 rotation 后原子写回并发布新 generation。

认证成功后的请求 span 记录 request id、user id、operation 和 Public Model；每次上游 attempt 记录 route、target、Provider family、
typed upstream operation 与脱敏
HTTP/transport 结果，终态 event 记录 HTTP status、response-ready、首 body 字节、SSE 首个 text/tool/reasoning token delta
增量、总耗时、retry/fallback/credential rotation/cooldown、取消/流失败和 Provider 明确返回的 usage。进程内累计值只
保留低基数请求终态、attempt 结果和 token 总量，并按 Provider attempt 记录性能、usage 与 cache 快照，可通过
`GatewayMetrics::snapshot`、`GatewayMetrics::provider_snapshots` 以及受 Bearer 保护的
`/openbridge/v1/metrics`、`/openbridge/v1/metrics/providers` 读取；详细口径见
[`遥测指标`](docs/implementation-status/telemetry-metrics.md)。OpenTelemetry/Prometheus exporter、 持久化和分布式聚合尚未实现。

## 验证基线

默认验证：

```bash
cargo fmt -- --check
cargo test --locked
cargo clippy --locked -- -D warnings
```

修改 `testdata/` 或 `tools/corpus/` 时，同时运行独立 Python corpus/testkit 基线：

```bash
uv lock --check --project tools/corpus
uv run --project tools/corpus pytest tools/corpus/tests
uv run --project tools/corpus corpus --root testdata lint
```

`tests/sdk_compatibility.rs` 使用运行时安装的当前 OpenAI Python 与 Node SDK 消费两个端点的 stream/non-stream、单/并行
function-tool 往返、流式 arguments 和 fixture 429 error：

```bash
cargo test --locked --test sdk_compatibility -- --ignored
```

不安装第三方 Python 包的 Embeddings discovery→request loopback：

```bash
cargo test --locked --test embedding_client_contract -- --ignored
```

这些 fixture 是确定性 wire regression。日常客户端可见行为优先使用 OpenAI SDK、独立 Python 脚本或 curl；只有当前行为明确以某个
Agent 为兼容目标时，才使用 Codex、Hermes 等客户端 runtime。SDK/工具不作长期版本固化，每次运行记录实际解析版本、安装来源、平台和无密钥配置。Windows
上可用 `OPENBRIDGE_NPM`/`OPENBRIDGE_NODE` 覆盖工具路径；也可用 `OPENBRIDGE_PNPM` 作为 Node SDK 的临时安装器。

独立的协议 corpus 维护说明见 [`testdata/`](testdata/README.md)，Mock Server/Client、单 case observation 判定、CLI 和
observation 说明见 [`tools/corpus/`](tools/corpus/README.md)。测试工具使用 `uv + Python` 维护，不读取 OpenBridge
配置，也不持有真实上游 credential。

## 推荐阅读顺序

| 文档                                                                                                       | 内容                                                               | 分类       |
|------------------------------------------------------------------------------------------------------------|--------------------------------------------------------------------|------------|
| [文档总索引](docs/README.md)                                                                               | 四类功能文档的统一入口                                             | 项目级入口 |
| [功能需求](docs/functional-requirements/README.md)                                                         | 产品范围、网关 API、配置凭证、路由韧性与交付证据                   | 功能需求   |
| [实施现状](docs/implementation-status/README.md)                                                           | 当前代码已证明行为、能力探测与验证记录                             | 实施现状   |
| [实施计划](docs/implementation-plans/README.md)                                                            | 唯一的短周期当前开发焦点                                           | 实施计划   |
| [参考文档](docs/references/README.md)                                                                      | OpenAI/OpenRouter 协议和参考项目事实                               | 参考文档   |
| [产品范围](docs/functional-requirements/product-scope.md)                                                  | 单配置所有者部署、下游用户、边界与非目标                           | 功能需求   |
| [网关 API 与客户端兼容](docs/functional-requirements/gateway-api-compatibility.md)                         | 下游 endpoint、原生 JSON/SSE、tool、continuation 与 Codex 扩展边界 | 功能需求   |
| [Embeddings 与 Native 多模态扩展](docs/functional-requirements/embedding-and-native-multimodal.md)         | 现阶段两个扩展目标的 wire、能力、资源与失败边界                    | 功能需求   |
| [Public Model 与模型能力契约](docs/functional-requirements/model-information-and-capability-contract.md)   | 模型信息、固定能力预检、Models API 与禁止能力路由边界              | 功能需求   |
| [Bootstrap、代码注册表、凭证与受信运行边界](docs/functional-requirements/configuration-and-credentials.md) | bootstrap、显式 Provider 注册、secret 与网络信任边界               | 功能需求   |
| [路由与 Provider 韧性](docs/functional-requirements/provider-resilience.md)                                | 固定 Route 顺序、状态亲和、限流、冷却、重试与错误传播              | 功能需求   |
| [当前实现说明](docs/implementation-status/current-implementation.md)                                       | 当前代码真正验证的行为和未证明事项                                 | 实施现状   |
| [遥测指标](docs/implementation-status/telemetry-metrics.md)                                                | Provider attempt 性能、usage、cache 指标口径和读取边界             | 实施现状   |
| [当前代码架构](docs/implementation-status/current-architecture.md)                                         | 按层次描述当前源码模块、请求路径、依赖和结构限制                   | 实施现状   |
| [当前开发焦点](docs/implementation-plans/current-focus.md)                                                 | 一个短周期行为的测试先行记录                                       | 实施计划   |
| [参考项目调研总览](docs/references/project-comparison.md)                                                  | Codex、Hermes、LiteLLM、cc-switch、CLIProxyAPI 等项目的证据范围    | 参考文档   |

文档分类与维护规则见 [`docs/README.md`](docs/README.md)。

## 当前非目标

- 多租户、团队成员、principal/ACL、面向下游用户/key 的配额、计费、合规审计和独立控制面；
- subscription/OAuth 多账号池、账号级负载均衡或动态 credential 控制面；
- OpenAI 全部资源 API、Realtime、Files、Conversations 或管理 API；
- 首版 Responses WebSocket transport；Codex 基线使用独立 custom Provider，并显式配置 `supports_websockets = false`；
- 将 Chat ↔ Responses 承诺为无损；不可表达的能力必须拒绝或显式标记；
- 让业务请求动态提供任意上游 URL、认证 header、credential、header 转换规则或转换脚本；
- 让 OpenBridge 执行 Agent 返回的通用 function tool；Protocol Bridge 只转换 wire-level tool call/result。
- GUI、Web 控制台、客户端注册/配置管理或面向用户的管理服务。

## 关键术语

- **Provider Family**：代码中实现的一类协议和认证行为，例如 `openai`、`openai-compatible`、`anthropic`。
- **Provider Instance**：Provider Family 的一个受信部署，唯一拥有一个 BaseURL；不同 URL 或区域注册为不同实例。
- **Credential Pool**：同一 Provider/credential kind 下可被多个 Target 共享的有序 API-key 集合。
- **Upstream Target**：引用一个 Provider Instance，并绑定 credential pool、Model、timeout 与故障边界的上游调用目标。
- **Upstream API**：Upstream Target 中由 `OperationKind` 唯一标识的原生供应，拥有 upstream model、限制、能力证据和 state affinity。
- **Public Model**：客户端使用的稳定模型身份、每协议唯一固定能力契约及私有有序 Route ID，例如 `gpt-5.6-sol`。
- **RoutePlan**：请求通过 Public Model 预检后固定的 Upstream Target/typed upstream operation、协议模式、credential pool binding、转换约束与
  fallback 边界；实际 member 由 attempt 选择。
- **Native path**：下游与上游协议一致时的最小改写转发路径，不经过通用 IR。
- **Protocol Bridge**：仅在协议不一致时使用的受限语义转换路径。
- **Tool Bridge**：把本地或 MCP 工具补充给 Agent；与 Protocol Bridge 不同。
- **Hosted Tool Facade**：将 Provider 原生托管工具规范化为独立工具接口；与普通 function tool 不等价。

## 证据和更新原则

- 官方 API、Codex 与 Hermes 当前行为优先以官方文档、源码和记录实际运行环境的 fixture 为准。
- 外部项目源码调研必须记录 repository、commit、文件范围、观察事实、推论和适用边界。
- 原型实验必须同时记录“证明什么”和“不证明什么”，避免代码存在本身形成架构结论。
- LiteLLM、cc-switch、CLIProxyAPI 等项目用于比较和寻找反例，不等同于 OpenBridge 的依赖或实现承诺。
- 每次目标客户端、SDK、Provider API 或规范升级后，应重新运行对应 corpus 和 Agent tool-loop fixture。

## 开源协议

OpenBridge 的原创源代码与随仓库提供的文档采用 [MIT License](LICENSE)。参考项目（包括 Codex）仅用于协议、
行为和实现边界的独立调研；本仓库不包含其派生代码。该声明不授予任何第三方材料的使用权：后续若引入外部
代码、测试或资源，必须在引入时保留其原有许可证、版权声明和适用的通知文件。
