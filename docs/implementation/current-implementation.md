# 当前实现说明

## 状态与范围

本文只描述当前 `main` 分支已经存在并有测试证据的行为；路线图中的 Phase 2–6 设计不因本文而成为可用功能。

- **Phase 0：已完成的基础**：严格 TOML 配置、loopback listener、origin allowlist、请求/SSE 大小上限、request id、共享 HTTP client、SSE framing、编译期 provider catalog 与 provider adapter 契约。
- **Phase 1：已完成**：一个 `openai` API-key upstream 的 Chat Completions/Responses 原生转发、静态下游 Bearer 认证、流式 conformance 和 OpenAI SDK compatibility fixture。
- **Phase 3：已完成的路由基线**：有序 deployment candidate、capability gate、`GET /v1/models` 和同协议、输出前的 streaming fallback。
- **未完成**：真实 Codex OAuth、第二 provider adapter、health/priority/weight 路由、principal 级授权、proxy-issued key、审计/指标、Chat ↔ Responses 转换和 Responses resource API。

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

业务 endpoint 只接受一个 `Content-Type: application/json`。未知 alias、缺失 `model`、非 JSON body、无能力 candidate 等错误在上游调用前返回 OpenAI-style JSON error envelope。`OPENBRIDGE_DOWNSTREAM_TOKEN` 是临时单一 credential；它不具备 Phase 4 的签发、撤销、principal 或授权范围能力。

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

## 已验证的契约

默认验证命令：

```bash
cargo fmt -- --check
cargo test --locked
cargo clippy --locked -- -D warnings
```

`tests/` 覆盖配置/allowlist/capability 上界、静态下游认证、原生请求改写、`/v1/models`、候选 fallback、provider-bound state、429 retry header、timeout、取消传播、断裂 UTF-8、EOF、partial-stream failure 与安全响应头。

`tests/sdk_compatibility.rs` 是 ignored integration test。它启动 loopback OpenBridge 与 mock upstream，再以 OpenAI Python `2.46.0` 和 Node `6.48.0` SDK 消费两个 endpoint 的 stream/non-stream fixture；fixture 包含断开的 UTF-8、多 event 同 chunk、单 event 跨 chunk 和多行 `data:`：

```bash
cargo test --locked --test sdk_compatibility -- --ignored
```

该测试不访问真实 provider。Windows 中若子进程无法从 `PATH` 找到工具，可通过 `OPENBRIDGE_UV`、`OPENBRIDGE_NPM` 与 `OPENBRIDGE_NODE` 指向可执行文件。

## 当前安全边界和限制

- 当前 bootstrap listener 强制为 loopback；在 Phase 4 前不得公开为共享 proxy。
- 当前只有 `ProviderKind::OpenAi` 和 API-key credential；真实 OAuth 不存在。
- 当前 route 配置可完整校验和原子 reload，但服务入口尚未暴露 reload 管理 API。
- 没有 audit outbox、指标、principal 授权、限流、health check 路由策略或真实多 provider health/fallback 证据。
- 不支持 Chat ↔ Responses conversion、Realtime、Files、Conversations、Responses retrieve/delete/background/cancel/store 等资源语义。

## 相关资源

- [项目入口](../../README.md)
- [开发计划](../plans/development-plan.md)
- [目标架构与路线](../architecture/architecture-and-roadmap.md)
- [Rust provider adapter 与数据流架构](../architecture/rust-provider-adapter-dataflow.md)
