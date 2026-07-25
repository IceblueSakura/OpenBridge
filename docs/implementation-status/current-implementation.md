# 当前实现说明

## 状态与范围

本文只描述当前 `main` 分支已经存在并有测试证据的行为。当前代码是**设计假设的实验性验证版本**：它能证明列出的 HTTP/SSE、路由和取消行为，不证明最终 Provider Family、配置 schema、目标客户端兼容范围或 Protocol Bridge 已经收敛。

- **已验证基础**：严格 TOML 配置、loopback listener、origin allowlist、受限 deployment 路径前缀、请求/SSE 大小上限、request id、共享 HTTP client、SSE framing、typed provider/route contracts。
- **已验证原生转发**：一个 `openai` API-key upstream 的 Chat Completions/Responses HTTP JSON/SSE 原生转发、静态下游 Bearer 认证、流式 conformance 和 OpenAI SDK compatibility fixture。
- **已验证路由基线**：有序 deployment candidate、capability gate、手工 `max_output_tokens` 候选筛选、下游 `GET /v1/models` 和同协议、输出前的 streaming fallback。
- **已验证显式探测**：受信 deployment 的上游模型列表、最小 Chat/Responses 请求，以及两种协议的 function-tool call/result replay 可输出不含敏感正文的观察报告；探测不改写配置。
- **尚未验证**：跨请求 deployment cooldown、配置化 retry budget/backoff、all-candidates-cooling-down 错误、第二 Provider Family、Codex CLI custom Provider E2E，以及任何已宣称 Hermes 兼容的 Agent tool loop、受信自定义 endpoint 的最终配置边界、Chat ↔ Responses bridge、Anthropic Messages、usage、hosted tool 和真实 OAuth。

## 运行模型

启动程序读取 `config/bootstrap.toml` 与 `config/routes.toml`，经 `config::load_registry` 校验后形成不可变 `RegistrySnapshot`。

配置代码已按“来源 → 文档 → 编译快照 → 路由消费”收敛：`ConfigPaths` 统一服务和 probe CLI 的
默认路径/环境覆盖及文件读取；私有 TOML 文档模型只负责 schema 反序列化；`RegistrySnapshot` 和
`ModelLimits` 是经过验证的运行时值。当前没有单独的配置 `Model` 实体；后续设计应在文档层新增
该实体并编译为稳定运行时元数据，而不是让 pipeline 直接解释 TOML。

- bootstrap policy 包含监听地址、允许的上游 origin、body/SSE 限制和连接池策略；当前只允许 loopback listener 和 HTTPS 根 origin。deployment 的 `base_url` 必须属于其中一个 origin，但可额外包含由受信配置提供的安全路径前缀（例如 `/openai`）。
- route 配置只能选择编译期存在的 `ProviderKind`、endpoint profile、credential reference、deployment 和 alias；不能定义任意 header、认证逻辑、请求转换或 URL。
- alias 的 `candidates` 是有序 deployment id 列表。请求取得 snapshot 后一直持有同一 `Arc`，未来 reload 不会改变正在运行的请求。
- 当前 `SecretReference` 仅接受 `env://NAME`，snapshot 不保存 API key 明文。业务请求时才由 `CredentialSource::Environment` 解析环境变量。

默认开发配置监听 `127.0.0.1:8080`，并将 `code-primary` 映射到 `openai-main`。启动需要：

```bash
export OPENBRIDGE_DOWNSTREAM_TOKEN='local-client-token'
export OPENAI_API_KEY='upstream-api-key'
cargo run --bin openbridge --locked
```

这两个值都不得写入配置文件、提交记录或普通日志。

## 下游 HTTP API

| Endpoint | 当前行为 | 认证 |
|---|---|---|
| `GET /healthz` | 返回 `{ "status": "ok", "config_version": "..." }` 和 proxy 生成的 `x-request-id`；不读取上游 credential。 | 无 |
| `GET /v1/models` | 返回配置中 public alias 的 OpenAI-style model list；不查询上游、不暴露 provider、deployment 或 credential id。 | 静态 Bearer |
| `POST /v1/chat/completions` | 原生转发 Chat JSON/SSE；只改写 `model`。 | 静态 Bearer |
| `POST /v1/responses` | 原生转发 Responses JSON/SSE；只改写 `model`。 | 静态 Bearer |

业务 endpoint 只接受一个 `Content-Type: application/json`。未知 alias、缺失 `model`、非 JSON body、无能力 candidate 等错误在上游调用前返回 OpenAI-style JSON error envelope。`OPENBRIDGE_DOWNSTREAM_TOKEN` 是面向单用户部署的单一静态 credential；当前没有多 key、签发、撤销列表、principal 或 scope。

## 请求、路由与上游调用

1. ingress 分配/传播 `x-request-id`，限制 request body，并用 constant-time 静态 Bearer 比较认证。
2. `pipeline::prepare_native_request` 解析 public `model`、`stream`、`tools`、显式并行工具、image content part、structured output、`previous_response_id`、`background`、`store` 和请求级输出上限等能力需求。
3. pipeline 依 alias 顺序筛选同时支持协议和所需 capability 的 deployment；为每个兼容 candidate 复制原 JSON，只把 `model` 改为该 deployment 的 `upstream_model`。
4. `ProviderAdapter::OpenAi` 生成固定的相对 path（`/v1/chat/completions` 或 `/v1/responses`）、`Content-Type` 与短时 Bearer 认证 header。
5. `UpstreamClient` 只把相对 URI 追加到配置验证过的 deployment endpoint base，禁用重定向并复用连接池。路径前缀只能来自 `base_url`；adapter 或业务请求不能覆盖它。

每个 deployment 可选地手工维护 `model_limits.context_window_tokens` 和 `model_limits.max_output_tokens`。当前只对后者的请求级显式值进行 egress 前筛选；上下文 token 的精确输入计数需要 future model-specific tokenizer，尚未实现。管理员可使用 `cargo run --bin openbridge-probe -- --deployment <id>` 显式探测受信上游的模型列表、Chat、Responses 和 function-tool loop；该命令不会改变下游模型列表或写回配置。

当前没有 health、priority 或 weight 策略；candidate 顺序就是确定性的优先级。`previous_response_id` 会关闭跨 candidate fallback，避免把 provider-bound continuation state 重放到另一个 deployment。

## 原生转发和流语义

非流式响应保留上游 HTTP status、body 和有限响应头。流式响应不做 Chat ↔ Responses 转换，也不重新生成 event；proxy 保留原始 SSE bytes，同时使用 `SseDecoder` 验证分片 UTF-8、行、空行 event boundary、多行 `data:` 与 event 大小上限。

- `Chat Completions` 以 `data: [DONE]` 识别完成。
- `Responses` 以 `response.completed`、`response.failed` 或 `response.incomplete` 识别终态。
- 合法 EOF 但没有 terminal event 时，已发出的 bytes 原样结束，并记录 warning；不会伪造 `response.completed` 或 `[DONE]`。
- 无效 UTF-8/SSE framing 或输出中的上游 body error 会关闭当前 stream；不会把已开始的 stream 与新尝试拼接。
- 下游丢弃 response body 时，包装 stream 和 reqwest bytes stream 一并 drop，以传播取消。

对 `stream=true`，429、5xx、连接错误或 timeout 仅在尚未把上游 response body 返回给下游时允许有限重试；每个 candidate 最多两次。耗尽同一 candidate 的可重试失败后，可进入下一个兼容 candidate（除 provider-bound state 外）。最终 HTTP 错误保留安全的 `Content-Type`、`Retry-After`、`x-should-retry`、`openai-request-id` 和 `x-ratelimit-*` 头；非 SSE 错误体不会被错误当成 SSE 解码。

这是单请求内的固定次数原型，不满足[Provider 韧性需求](../functional-requirements/provider-resilience.md)新增的跨请求 cooldown、可取消 backoff、次数/等待/总耗时联合预算、重复安全性分类和全部 candidate cooling down 错误契约。

## 已验证的契约

默认验证命令：

```bash
cargo fmt -- --check
cargo test --locked
cargo clippy --locked -- -D warnings
```

`tests/` 覆盖配置/allowlist/capability 上界、静态下游认证、原生请求改写、`/v1/models`、候选 fallback、provider-bound state、429 retry header、timeout、取消传播、断裂 UTF-8、EOF、partial-stream failure 与安全响应头。

`tests/sdk_compatibility.rs` 是 ignored integration test。它启动 loopback OpenBridge 与 mock upstream，再以运行时安装的 OpenAI Python 和 Node SDK 消费两个 endpoint 的 stream/non-stream、单/并行 function-tool call/result 往返、流式 arguments 及 fixture 429 error；fixture 包含断开的 UTF-8、多 event 同 chunk、单 event 跨 chunk 和多行 `data:`：

```bash
cargo test --locked --test sdk_compatibility -- --ignored
```

[`tools/upstream-fixture-server`](../../tools/upstream-fixture-server/README.md) 是与被测 transport 分离的 loopback 测试上游。它在 `mock` 模式离线提供两个 endpoint 的确定性 JSON/SSE/429；在显式 `proxy` 模式从被忽略的 `tools/upstream-fixture-server/.env` 或进程环境读取 `UPSTREAM_FIXTURE_API_BASE`、`UPSTREAM_FIXTURE_API_KEY` 与可选 `UPSTREAM_FIXTURE_MODEL`，以原生请求体访问授权真实上游。默认模型只补全缺失字段，不覆盖调用方模型；它不 bridge 协议、不记录 API key 或 request/response body，也不等同于真实 Provider 或真实 Agent client 观察。

该测试是日常 SDK wire regression，不访问真实 provider，也不代表 Codex CLI custom Provider 或已宣称的 Hermes 兼容性已经得到验证。Windows 中若子进程无法从 `PATH` 找到工具，可通过 `OPENBRIDGE_UV`、`OPENBRIDGE_NPM` 与 `OPENBRIDGE_NODE` 指向可执行文件；`OPENBRIDGE_PNPM` 可作为 Node SDK 的临时安装器。

2026-07-24 的直接真实 Provider 验证使用含非根路径前缀的受信 `base_url`，Chat/Responses 的 JSON 与 SSE 均返回成功；未固定版本的 OpenAI Python SDK `2.48.0` 也完成两种协议的 stream/non-stream 解析。测试只记录协议终态和断言结果，不保存 credential 或上游正文。它证明该 endpoint 形状可用，不代表工具调用、跨 Provider fallback 或其他 capability 已获真实 Provider 验证。

## 当前安全边界和限制

- 当前 bootstrap listener 强制为 loopback。未来允许非 loopback 时，必须至少要求静态高熵 token，并由 TLS 或可信反向代理保护。
- 当前只有 `ProviderKind::OpenAi` 和 API-key credential；真实 OAuth 不存在。
- 当前 route 配置可完整校验和原子 reload，但服务入口尚未暴露 reload 管理 API。
- 没有 usage sink、跨请求 deployment cooldown、第二 Provider Family 或真实多 Provider fallback 证据；多租户授权、面向下游用户/key 的配额和合规审计不属于当前核心目标。
- 不支持 Chat ↔ Responses conversion、Responses WebSocket、Realtime、Files、Conversations、Responses retrieve/delete/background/cancel/store 等资源语义。

## 相关资源

- [项目入口](../../README.md)
- [交付与证据要求](../functional-requirements/delivery-and-evidence.md)
- [当前开发焦点](../implementation-plans/current-focus.md)
- [服务架构](../implementation-plans/service-architecture.md)
- [Provider 适配与数据流](../implementation-plans/provider-adapters-and-dataflow.md)
- [客户端兼容](../implementation-plans/client-compatibility.md)
- [Provider 韧性](../functional-requirements/provider-resilience.md)
- [配置与路由](../implementation-plans/configuration-and-routing.md)
- [上游模型发现与能力探测](capability-probing.md)
