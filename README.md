# OpenBridge 设计与调研索引

## 目标

构建一个面向 OpenAI-compatible 客户端的 proxy，逐步支持：

1. 原生转发 `POST /v1/chat/completions` 和 `POST /v1/responses`；
2. proxy 自主管理单一 Codex OAuth credential（真实 OAuth 契约与条款 preflight 是硬门）；
3. 多 provider、多 deployment 与稳定模型别名；
4. Rust 编译期 provider adapter 与有类型异步数据流 pipeline；
5. proxy 自行签发的 API key 校验和授权；
6. 隐私可控的请求审计与运行日志；
7. Chat Completions 与 Responses 的双向协议转换。

本项目已完成 **Phase 1 单上游原生转发**，并正在推进 Phase 3 路由基线。已实现严格配置、不可变 route snapshot、OpenAI-compatible Chat/Responses 原生转发、静态下游 Bearer 认证、共享 upstream 连接池、下游断开时的上游 stream 取消传播、仅在下游业务 SSE 前执行的有界 retry、按实际 SSE response 进行 framing 校验，以及有序多 deployment candidate、capability gate、受保护的 `/v1/models` 与同协议 streaming fallback。Phase 1 conformance 覆盖 429/5xx、timeout、EOF、partial-stream failure、断开的 UTF-8、多 event 同 chunk、跨 chunk event 和多行 `data:`；OpenAI Python `2.46.0` 与 Node `6.48.0` SDK 的两端点 stream/non-stream loopback fixture 已通过。真正的多 provider catalog、health/weight 路由、OAuth、审计与协议转换仍未完成，因此不代表生产可用。

文档目录说明见 [`docs/README.md`](docs/README.md)。

## 当前可运行基线

仓库内的 [`config/bootstrap.toml`](config/bootstrap.toml) 和 [`config/routes.toml`](config/routes.toml) 是无明文凭证的开发配置。启动服务：

```bash
export OPENBRIDGE_DOWNSTREAM_TOKEN='replace-with-a-local-client-token'
export OPENAI_API_KEY='replace-with-an-upstream-api-key'
cargo run --locked
```

`OPENBRIDGE_DOWNSTREAM_TOKEN` 在启动时读取一次，当前仅作为临时静态下游 Bearer credential；它不是 Phase 4 设计的可签发、可撤销 proxy key。`OPENAI_API_KEY` 由 [`config/routes.toml`](config/routes.toml) 中的 `env://OPENAI_API_KEY` binding 在业务请求时解析。两者都不得写入仓库或普通日志。

默认监听 `127.0.0.1:8080`。健康检查：

```bash
curl -i http://127.0.0.1:8080/healthz
```

响应只包含状态和当前配置版本，并生成 `x-request-id`。配置文件路径可通过 `OPENBRIDGE_BOOTSTRAP_CONFIG` 和 `OPENBRIDGE_ROUTES_CONFIG` 覆盖；`RUST_LOG` 控制日志过滤。健康检查公开且不会解析 upstream credential。

原生请求示例：

```bash
curl http://127.0.0.1:8080/v1/chat/completions \
  -H 'Authorization: Bearer replace-with-a-local-client-token' \
  -H 'Content-Type: application/json' \
  -d '{"model":"code-primary","messages":[{"role":"user","content":"hello"}]}'
```

`POST /v1/chat/completions` 与 `POST /v1/responses` 使用 alias 选择预配置 deployment，只将请求中的 `model` 改写为 `upstream_model`；其余 JSON 字段和 upstream JSON/SSE body 原生转发，不做 Chat ↔ Responses 转换。客户端不能指定 upstream URL 或任意出站 header。

## SDK compatibility fixture

`tests/sdk_compatibility.rs` 会启动 loopback proxy 与 mock upstream，然后使用 OpenAI Python `2.46.0` 和 Node `6.48.0` SDK 消费 Chat/Responses 的 stream 与 non-stream fixture。stream fixture 特意覆盖断开的 UTF-8、多 event 同 chunk、单 event 跨 chunk 及多行 `data:`。它是 ignored integration test，以免默认 `cargo test` 下载 SDK：

```bash
cargo test --locked --test sdk_compatibility -- --ignored
```

测试不访问真实 provider，也不需要真实 credential。它使用 `uv` 临时解析 Python SDK，并在系统临时目录安装 Node SDK；如果 Windows 子进程无法从当前 `PATH` 解析工具，可用 `OPENBRIDGE_UV`、`OPENBRIDGE_NPM`、`OPENBRIDGE_NODE` 指向对应可执行文件。

## 推荐阅读顺序

| 文档 | 内容 | 状态 |
|---|---|---|
| [初版需求](docs/requirements/proxy-requirements.md) | 产品范围、功能/安全/兼容性需求、初始验收集与调研 backlog | 初稿，待确认 |
| [当前实现说明](docs/implementation/current-implementation.md) | 当前代码、API、配置、路由、SSE 语义、测试证据与未实现边界 | 已同步 |
| [架构与路线](docs/architecture/architecture-and-roadmap.md) | 目标架构、控制面/数据面边界、分阶段开发门与验收标准 | 已同步 |
| [开发计划](docs/plans/development-plan.md) | 已确认的可执行开发计划、阶段任务、退出条件、风险与非目标 | 实施中 |
| [Rust provider adapter 与数据流](docs/architecture/rust-provider-adapter-dataflow.md) | Rust trait adapter、编译期 provider catalog、数据流 pipeline、配置边界与性能门 | 实施中 |
| [Codex OAuth 凭证边界](docs/design/codex-oauth-credential-boundary.md) | proxy 自主管理单一 Codex OAuth credential 的边界、生命周期与 preflight | 已同步 |
| [控制面、模型、密钥与可观测性](docs/architecture/control-plane-models-keys-and-observability.md) | 模型别名/路由、proxy-issued API key、审计和日志设计 | 目标设计，待实施 |
| [Hermes Agent 协议分析](docs/research/hermes/chat-responses-analysis.md) | Hermes 的 Chat/Responses adapter 与 continuation state | 已有 |
| [LiteLLM 协议分析](docs/research/litellm/chat-responses-analysis.md) | LiteLLM 的双向 bridge 和 provider gateway | 已有 |
| [Chat/Responses 转换设计](docs/design/chat-responses-conversion.md) | Canonical IR、转换器与 stream state machine 建议 | 已有 |
| [OpenAI API 规范目录](docs/specifications/openai/api-specification-catalog.md) | OpenAI 端点和规范地图 | 已有 |
| [Chat Completions 协议](docs/specifications/openai/chat-completions-protocol.md) | Chat Completions wire contract | 已有 |
| [Responses 协议](docs/specifications/openai/responses-protocol.md) | Responses wire contract 与资源生命周期 | 已有 |
| [LiteLLM Proxy 调用链](docs/research/litellm/proxy-call-chain-analysis.md) | LiteLLM Proxy 调用链 | 已有 |
| [LiteLLM Proxy 性能分析](docs/research/litellm/proxy-performance-bottlenecks.md) | LiteLLM 性能观察；仅作参考，非本项目现状 | 已有 |

## 非目标（当前阶段）

- 不代理 OpenAI 的全部资源 API、Realtime、Files、Conversations 或管理面。
- 不把 Chat ↔ Responses 转换承诺为无损；每个语义降级必须可检测。
- 不把 Codex 的本地 auth cache、ChatGPT access token 或 refresh token 暴露给下游客户端。
- 不允许客户端通过 `base_url`、任意 header 或模型名指定任意上游 URL/凭证。
- 不在未完成入站认证和最小审计前暴露到非受信网络。

## 关键术语

- **Public model / alias**：客户端请求的稳定模型名，例如 `code-primary`；不等于 provider 原始模型名。
- **Deployment**：一个可调用上游目标，绑定 provider、上游 model、endpoint、credential binding 和 capabilities。
- **Credential binding**：仅由控制面引用的上游认证材料；数据面不得接收或返回其值。
- **Proxy-issued opaque key**：proxy 自行生成、可撤销的高熵 bearer key；它不是 JWT，也不是上游 OpenAI/Codex OAuth token。
- **Canonical IR**：Chat 和 Responses 之间转换时使用的有序、异构内部表示；详见 [Chat/Responses 转换设计](docs/design/chat-responses-conversion.md)。

## 证据和更新原则

- OpenAI HTTP/SSE 行为以官方 API Reference、guides 和 OpenAPI 为准。
- Codex 登录的用户行为以官方 Codex 文档为准；未公开的 ChatGPT backend 协议不得作为稳定依赖。
- LiteLLM/Hermes/Codex 的源码分析是设计参考，不等同于本项目的依赖或实现承诺。
- 每次上游 SDK/OpenAPI 更新后，应运行兼容 fixture 并更新文档的版本/日期，而不是按模型名称猜测能力。
