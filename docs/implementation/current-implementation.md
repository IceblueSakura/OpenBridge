# 当前实现说明

## 状态与范围

本文只描述当前 `main` 分支已经存在并有测试证据的行为。当前代码是**设计假设的实验性验证版本**：它能证明列出的 HTTP/SSE、路由和取消行为，不证明最终 Provider Family、配置 schema、目标客户端兼容范围或 Protocol Bridge 已经收敛。

- **已验证基础**：严格 TOML 配置、loopback listener、origin allowlist、请求/SSE 大小上限、request id、共享 HTTP client、SSE framing、typed provider/route contracts。
- **已验证原生转发**：一个 `openai` API-key upstream 的 Chat Completions/Responses HTTP JSON/SSE 原生转发、静态下游 Bearer 认证、流式 conformance 和 OpenAI SDK compatibility fixture。
- **已验证路由基线**：有序 deployment candidate、capability gate、`GET /v1/models` 和同协议、输出前的 streaming fallback。
- **尚未验证**：跨请求 deployment cooldown、配置化 retry budget/backoff、all-candidates-cooling-down 错误、第二 Provider Family、Codex/Hermes 真实 Agent tool loop、受信自定义 endpoint 的最终配置边界、Chat ↔ Responses bridge、Anthropic Messages、usage、hosted tool 和真实 OAuth。

## 运行模型

启动程序读取 `config/bootstrap.toml` 与 `config/routes.toml`，经 `config::load_registry` 校验后形成不可变 `RegistrySnapshot`。

- bootstrap policy 包含监听地址、允许的上游 origin、body/SSE 限制和连接池策略；当前只允许 loopback listener 和 HTTPS 根 origin。
- route 配置只能选择编译期存在的 `ProviderKind`、endpoint profile、credential reference、deployment 和 alias；不能定义任意 header、认证逻辑、请求转换或 URL。
- alias 的 `candidates` 是有序 deployment id 列表。请求取得 snapshot 后一直持有同一 `Arc`，未来 reload 不会改变正在运行的请求。
- 当前 `SecretReference` 仅接受 `env://NAME`，snapshot 不保存 API key 明文。业务请求时才由 `CredentialSource::Environment` 解析环境变量。

默认开发配置监听 `127.0.0.1:8080`，并将 `code-primary` 映射到 `openai-main`。启动需要：

```bash
export OPENBRIDGE_DOWNSTREAM_TOKEN='local-client-token'
export OPENAI_API_KEY='upstream-api-key'
cargo run --locked
```

这两个值都不得写入配置文件、提交记录或普通日志。

## 下游 HTTP API

| Endpoint | 当前行为 | 认证 |
|---|---|---|
| `GET /healthz` | 返回 `{ "status": "ok", "config_version": "..." }` 和 proxy 生成的 `x-request-id`；不读取上游 credential。 | 无 |
| `GET /v1/models` | 返回配置中 public alias 的 OpenAI-style model list；不暴露 provider、deployment 或 credential id。 | 静态 Bearer |
| `POST /v1/chat/completions` | 原生转发 Chat JSON/SSE；只改写 `model`。 | 静态 Bearer |
| `POST /v1/responses` | 原生转发 Responses JSON/SSE；只改写 `model`。 | 静态 Bearer |

业务 endpoint 只接受一个 `Content-Type: application/json`。未知 alias、缺失 `model`、非 JSON body、无能力 candidate 等错误在上游调用前返回 OpenAI-style JSON error envelope。`OPENBRIDGE_DOWNSTREAM_TOKEN` 是面向单用户部署的单一静态 credential；当前没有多 key、签发、撤销列表、principal 或 scope。

## 请求、路由与上游调用

1. ingress 分配/传播 `x-request-id`，限制 request body，并用 constant-time 静态 Bearer 比较认证。
2. `pipeline::prepare_native_request` 解析 public `model`、`stream`、`tools`、structured output、`previous_response_id`、`background` 和 `store` 等能力需求。
3. pipeline 依 alias 顺序筛选同时支持协议和所需 capability 的 deployment；为每个兼容 candidate 复制原 JSON，只把 `model` 改为该 deployment 的 `upstream_model`。
4. `ProviderAdapter::OpenAi` 生成固定的相对 path（`/v1/chat/completions` 或 `/v1/responses`）、`Content-Type` 与短时 Bearer 认证 header。
5. `UpstreamClient` 只把相对 URI 拼接到配置 allowlist 中的 origin，禁用重定向并复用连接池。

当前没有 health、priority 或 weight 策略；candidate 顺序就是确定性的优先级。`previous_response_id` 会关闭跨 candidate fallback，避免把 provider-bound continuation state 重放到另一个 deployment。

## 原生转发和流语义

非流式响应保留上游 HTTP status、body 和有限响应头。流式响应不做 Chat ↔ Responses 转换，也不重新生成 event；proxy 保留原始 SSE bytes，同时使用 `SseDecoder` 验证分片 UTF-8、行、空行 event boundary、多行 `data:` 与 event 大小上限。

- `Chat Completions` 以 `data: [DONE]` 识别完成。
- `Responses` 以 `response.completed`、`response.failed` 或 `response.incomplete` 识别终态。
- 合法 EOF 但没有 terminal event 时，已发出的 bytes 原样结束，并记录 warning；不会伪造 `response.completed` 或 `[DONE]`。
- 无效 UTF-8/SSE framing 或输出中的上游 body error 会关闭当前 stream；不会把已开始的 stream 与新尝试拼接。
- 下游丢弃 response body 时，包装 stream 和 reqwest bytes stream 一并 drop，以传播取消。

对 `stream=true`，429、5xx、连接错误或 timeout 仅在尚未把上游 response body 返回给下游时允许有限重试；每个 candidate 最多两次。耗尽同一 candidate 的可重试失败后，可进入下一个兼容 candidate（除 provider-bound state 外）。最终 HTTP 错误保留安全的 `Content-Type`、`Retry-After`、`x-should-retry`、`openai-request-id` 和 `x-ratelimit-*` 头；非 SSE 错误体不会被错误当成 SSE 解码。

这是单请求内的固定次数原型，不满足[Provider 韧性需求](../requirements/provider-resilience.md)新增的跨请求 cooldown、可取消 backoff、次数/等待/总耗时联合预算、重复安全性分类和全部 candidate cooling down 错误契约。

## 已验证的契约

默认验证命令：

```bash
cargo fmt -- --check
cargo test --locked
cargo clippy --locked -- -D warnings
```

`tests/` 覆盖配置/allowlist/capability 上界、静态下游认证、原生请求改写、`/v1/models`、候选 fallback、provider-bound state、429 retry header、timeout、取消传播、断裂 UTF-8、EOF、partial-stream failure 与安全响应头。

`tests/sdk_compatibility.rs` 是 ignored integration test。它启动 loopback OpenBridge 与 mock upstream，再以 OpenAI Python `2.46.0` 和 Node `6.48.0` SDK 消费两个 endpoint 的 stream/non-stream、单/并行 function-tool call/result 往返、流式 arguments 及 fixture 429 error；fixture 包含断开的 UTF-8、多 event 同 chunk、单 event 跨 chunk 和多行 `data:`：

```bash
cargo test --locked --test sdk_compatibility -- --ignored
```

[`tools/upstream-fixture-server`](../../tools/upstream-fixture-server/README.md) 是与被测 transport 分离的 loopback 验收上游。它在 `mock` 模式离线提供两个 endpoint 的确定性 JSON/SSE/429；在显式 `proxy` 模式从被忽略的 `tools/upstream-fixture-server/.env` 或进程环境读取 `UPSTREAM_FIXTURE_API_BASE`、`UPSTREAM_FIXTURE_API_KEY` 与可选 `UPSTREAM_FIXTURE_MODEL`，以原生请求体访问授权真实上游。默认模型只补全缺失字段，不覆盖调用方模型；它不 bridge 协议、不记录 API key 或 request/response body，也不等同于真实 Provider 或真实 Agent client 验收。

该测试不访问真实 provider，也不代表 Codex/Hermes 已验收。Windows 中若子进程无法从 `PATH` 找到工具，可通过 `OPENBRIDGE_UV`、`OPENBRIDGE_NPM` 与 `OPENBRIDGE_NODE` 指向可执行文件；`OPENBRIDGE_PNPM` 可作为 Node SDK 的临时安装器。

## 当前安全边界和限制

- 当前 bootstrap listener 强制为 loopback。未来允许非 loopback 时，必须至少要求静态高熵 token，并由 TLS 或可信反向代理保护。
- 当前只有 `ProviderKind::OpenAi` 和 API-key credential；真实 OAuth 不存在。
- 当前 route 配置可完整校验和原子 reload，但服务入口尚未暴露 reload 管理 API。
- 没有 usage sink、跨请求 deployment cooldown、第二 Provider Family 或真实多 Provider fallback 证据；多租户授权、面向下游用户/key 的配额和合规审计不属于当前核心目标。
- 不支持 Chat ↔ Responses conversion、Responses WebSocket、Realtime、Files、Conversations、Responses retrieve/delete/background/cancel/store 等资源语义。

## 相关资源

- [项目入口](../../README.md)
- [阶段交付与研究需求](../requirements/delivery-requirements.md)
- [当前阶段实施计划](../plans/implementation-plan.md)
- [目标架构与路线](../architecture/architecture-and-roadmap.md)
- [Rust Provider adapter 与数据流架构](../architecture/rust-provider-adapter-dataflow.md)
- [目标客户端契约](../design/target-client-contracts.md)
- [Provider 韧性需求](../requirements/provider-resilience.md)
- [本地配置、路由与使用量](../architecture/local-configuration-routing-and-usage.md)
