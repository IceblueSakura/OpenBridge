# OpenBridge 设计与调研索引

## 项目定位

OpenBridge 的核心是一个**单用户、单服务的多 Provider Agent API 聚合代理**：部署在本地或用户自有云环境中，集中管理上游 Provider、凭证、模型与路由，并向 Codex、Hermes Agent 等客户端提供稳定的 OpenAI-compatible 接口。

当前处于**设计探索与原型验证阶段**。仓库中的 Rust 代码用于验证 HTTP/SSE、路由快照、能力检查和 fallback 等关键假设，不代表最终模块边界、Provider 抽象或协议桥接方案已经收敛。

核心方向：

1. 原生转发 `POST /v1/responses` 与 `POST /v1/chat/completions` 的 HTTP JSON/SSE；
2. 聚合多个 Provider、deployment 与稳定模型 alias；
3. 以编译期 Provider Family 承载协议行为，以受信运行时配置定义 deployment；
4. 在原生协议不可用时，对明确支持的语义执行 Chat ↔ Responses bridge；
5. 正确处理 SSE、tool-call identity、continuation state、取消与首输出前 fallback；
6. 优先保证 Codex 自定义 Provider 的 Responses HTTP/SSE profile 与 Hermes Chat/Responses 的真实 Agent tool loop 兼容性。

核心稳定后再考虑：

- Provider-hosted tool facade；
- 本地/MCP Tool Bridge；
- 使用量与成本分析；
- 可选 OAuth credential adapter；
- 简单管理界面与更多路由策略。

## 当前可运行基线

当前 `main` 已实现一个 OpenAI API-key upstream 的 Chat/Responses HTTP JSON/SSE 原生转发，以及有序 deployment candidate、capability gate、受保护的 `/v1/models`、输出前 retry/fallback、SSE framing 校验和下游断开时的上游 stream 取消传播。

仓库内的 [`config/bootstrap.toml`](config/bootstrap.toml) 和 [`config/routes.toml`](config/routes.toml) 是无明文凭证的开发配置：

```bash
export OPENBRIDGE_DOWNSTREAM_TOKEN='replace-with-a-local-client-token'
export OPENAI_API_KEY='replace-with-an-upstream-api-key'
cargo run --locked
```

默认监听 `127.0.0.1:8080`。健康检查：

```bash
curl -i http://127.0.0.1:8080/healthz
```

原生请求示例：

```bash
curl http://127.0.0.1:8080/v1/chat/completions \
  -H 'Authorization: Bearer replace-with-a-local-client-token' \
  -H 'Content-Type: application/json' \
  -d '{"model":"code-primary","messages":[{"role":"user","content":"hello"}]}'
```

当前只改写 `model` 并使用预配置 deployment；其余 JSON 与上游 JSON/SSE body 原生转发，不做 Chat ↔ Responses 转换。客户端不能通过业务请求指定上游 URL、credential 或任意出站 header。

## 验证基线

默认验证：

```bash
cargo fmt -- --check
cargo test --locked
cargo clippy --locked -- -D warnings
```

`tests/sdk_compatibility.rs` 使用 OpenAI Python `2.46.0` 与 Node `6.48.0` SDK 消费两个端点的 stream/non-stream loopback fixture：

```bash
cargo test --locked --test sdk_compatibility -- --ignored
```

这些 fixture 证明特定模拟输出可被对应 SDK 消费；它们不替代真实 Provider corpus、Codex/Hermes 完整 tool loop 或异构协议 bridge 验证。

## 推荐阅读顺序

| 文档 | 内容 | 状态 |
|---|---|---|
| [文档总索引](docs/README.md) | 按功能模块和实施阶段组织全部文档 | 项目级入口 |
| [功能模块索引](docs/modules/README.md) | 产品边界、客户端、路由、Provider、Native、Bridge、安全与增强 | 当前功能视图 |
| [实施阶段索引](docs/phases/README.md) | C0–C6 和增强阶段的目标、测试与退出条件 | 当前实施视图 |
| [核心需求](docs/requirements/proxy-requirements.md) | 单用户部署、核心范围、非目标与验收方向 | 工作基线，待调研收敛 |
| [目标客户端契约](docs/design/target-client-contracts.md) | Codex 与 Hermes 的协议优先级、测试矩阵和版本固定规则 | 工作假设 |
| [目标架构与路线](docs/architecture/architecture-and-roadmap.md) | 单服务架构、原生/桥接双路径、路由与状态边界 | 工作假设 |
| [Rust Provider adapter 与数据流](docs/architecture/rust-provider-adapter-dataflow.md) | Provider Family、deployment 配置、typed pipeline 与 conformance | 工作假设，原型部分验证 |
| [本地配置、路由与使用量](docs/architecture/local-configuration-routing-and-usage.md) | 单用户配置模型、alias、静态入站 token 与可选 usage sink | 目标设计 |
| [Chat/Responses bridge](docs/design/chat-responses-conversion.md) | bridge-only IR、状态机、tool identity 与降级边界 | 工作假设 |
| [开发与调研收敛计划](docs/plans/development-plan.md) | 调研问题、实验、决策门和候选实施顺序 | 实施中 |
| [参考项目比较矩阵](docs/research/project-comparison-matrix.md) | Codex、Hermes、LiteLLM、cc-switch、Bifrost、CLIProxyAPI 的研究职责 | 持续更新 |
| [当前实现说明](docs/implementation/current-implementation.md) | 当前代码真正验证的行为和未证明事项 | 已同步 |
| [Hosted tool 增强需求](docs/requirements/hosted-tools-mcp.md) | 核心稳定后的 Provider-hosted tool facade | 延期增强 |
| [Codex OAuth 凭证边界](docs/design/codex-oauth-credential-boundary.md) | 可选 OAuth adapter 的安全边界与 preflight | 延期/受外部契约阻塞 |

文档目录说明见 [`docs/README.md`](docs/README.md)。

## 当前非目标

- 多租户、团队成员、principal/ACL、配额、计费、合规审计和独立控制面；
- 同 Provider 多账号池、credential 轮换池或账号级负载均衡；
- OpenAI 全部资源 API、Realtime、Files、Conversations 或管理 API；
- 首版 Responses WebSocket transport；Codex 基线使用独立 custom Provider，并显式配置 `supports_websockets = false`；
- 将 Chat ↔ Responses 承诺为无损；不可表达的能力必须拒绝或显式标记；
- 让业务请求动态提供任意上游 URL、认证 header、credential 或转换脚本；
- 让 OpenBridge 执行 Agent 返回的通用 function tool；Protocol Bridge 只转换 wire-level tool call/result。

## 关键术语

- **Provider Family**：代码中实现的一类协议和认证行为，例如 `openai`、`openai-compatible`、`anthropic`。
- **Deployment**：受信配置中的一个上游目标，绑定 Provider Family、base URL、credential reference、上游模型和能力。
- **Public model alias**：客户端使用的稳定模型名，例如 `code-primary`；映射到有序 deployment candidates。
- **RoutePlan / RouteSnapshot**：单次请求固定的 deployment、协议模式、能力判断、credential binding 与 fallback 边界。
- **Native path**：下游与上游协议一致时的最小改写转发路径，不经过通用 IR。
- **Protocol Bridge**：仅在协议不一致时使用的受限语义转换路径。
- **Tool Bridge**：把本地或 MCP 工具补充给 Agent；与 Protocol Bridge 不同。
- **Hosted Tool Facade**：将 Provider 原生托管工具规范化为独立工具接口；与普通 function tool 不等价。

## 证据和更新原则

- 官方 API、Codex 与 Hermes 当前行为优先以官方文档、源码和固定版本 fixture 为准。
- 外部项目源码调研必须记录 repository、commit、文件范围、观察事实、推论和适用边界。
- 原型实验必须同时记录“证明什么”和“不证明什么”，避免代码存在本身形成架构结论。
- LiteLLM、cc-switch、Bifrost、CLIProxyAPI 等项目用于比较和寻找反例，不等同于 OpenBridge 的依赖或实现承诺。
- 每次目标客户端、SDK、Provider API 或规范升级后，应重新运行对应 corpus 和 Agent tool-loop fixture。
