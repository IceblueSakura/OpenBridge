# OpenBridge 项目说明

## 项目定位

OpenBridge 的核心是一个**单用户、单服务、headless 的多 Provider Agent API 聚合网关**：部署在本地或用户自有云环境中，通过显式 Rust 代码注册表管理上游 Provider、模型与路由，通过受限 secret source 获取凭证，并向本地正在使用的 Codex、Hermes Agent 等客户端提供稳定的 OpenAI-compatible 接口。它不提供 GUI、Web 控制台或客户端管理功能。

当前处于**设计探索与原型验证阶段**。仓库中的 Rust 代码用于验证 HTTP/SSE、路由快照、能力检查和 fallback 等关键假设，不代表最终模块边界、Provider 抽象或协议桥接方案已经收敛。开发采用 TDD：每次只选择一个可观察行为，先写会失败的测试，再以最小实现使其通过。

核心方向：

1. 原生转发 `POST /v1/responses` 与 `POST /v1/chat/completions` 的 HTTP JSON/SSE；
2. 聚合多个 Provider、deployment 与稳定模型 alias；
3. 以每 Provider 独立 Rust 模块承载协议行为、模型、deployment 和能力，以显式注册表统一发现；
4. 在原生协议不可用时，对明确支持的语义执行 Chat ↔ Responses bridge；
5. 正确处理 SSE、tool-call identity、continuation state、取消、有限 retry、deployment cooldown、首输出前 fallback 与最终错误传播；
6. 优先保证 Codex 自定义 Provider 的 Responses HTTP/SSE profile；Hermes 的真实 Agent tool loop 只在明确宣称兼容时验证。
7. 以 bootstrap-only 配置管理进程资源策略，以外部 secret source 管理上下游 credential，并通过 headless 输出提供调用量、usage、TTFT/TTFB 和终态错误率统计。

核心稳定后再考虑：

- Provider-hosted tool facade；
- Anthropic Messages 协议兼容与异构 Provider 验证（与 Provider-hosted tool facade 同级）；
- 本地/MCP Tool Bridge；
- headless 的健康、日志与诊断；
- 可选 OAuth credential adapter；
- 更多路由策略。

## 当前可运行基线

当前 `main` 已实现一个 OpenAI API-key upstream 的 Chat/Responses HTTP JSON/SSE 原生转发，以及有序 deployment candidate、capability gate、受保护的 `/v1/models`、输出前 retry/fallback、SSE framing 校验和下游断开时的上游 stream 取消传播。

仓库内的 [`config/bootstrap.toml`](config/bootstrap.toml) 只配置监听和资源限制；canonical Model 位于 [`src/models`](src/models)，Provider 与 Deployment 位于 [`src/providers`](src/providers)，public alias 由顶层代码注册表显式组合：

```bash
cp .env.example .env
# 编辑 .env，至少设置 OPENBRIDGE_DOWNSTREAM_TOKEN，并填写实际使用 Provider 的 API key。
cargo run --bin openbridge --locked
```

服务与 `openbridge-probe` 会可选加载当前目录或父目录中的 `.env`；已有进程环境变量优先。
`.env` 已被 Git 忽略，仓库只提交不含真实凭证的 [`.env.example`](.env.example)。

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

当前由 Provider adapter 写入实际上游 `model`；其余 JSON 与上游 JSON/SSE body 原生转发，不做 Chat ↔ Responses 转换。客户端不能通过业务请求指定上游 URL、credential 或任意出站 header。

当前凭证基线使用环境变量：代码注册表只保存环境变量名称，不保存 secret。以后可增加 keyring 或受限私有 secret 文件，但不会恢复运行时 Provider DSL 或 route 热重载。调用量、Provider usage、TTFT/TTFB 和终态错误率属于后续 headless 统计能力，当前尚未实现。

## 验证基线

默认验证：

```bash
cargo fmt -- --check
cargo test --locked
cargo clippy --locked -- -D warnings
```

`tests/sdk_compatibility.rs` 使用运行时安装的当前 OpenAI Python 与 Node SDK 消费两个端点的 stream/non-stream、单/并行 function-tool 往返、流式 arguments 和 fixture 429 error：

```bash
cargo test --locked --test sdk_compatibility -- --ignored
```

这些 fixture 是确定性 wire regression。日常行为验证优先使用 OpenAI SDK 与 Codex CLI；真实 Provider corpus 用于定位 Provider 特有问题，Hermes 只在明确宣称兼容时纳入验证。SDK/CLI 不作长期版本固化，每次运行记录实际解析版本、安装来源、平台和无密钥配置。Windows 上可用 `OPENBRIDGE_NPM`/`OPENBRIDGE_NODE` 覆盖工具路径；也可用 `OPENBRIDGE_PNPM` 作为 Node SDK 的临时安装器。

独立的协议 corpus、增量 SSE parser、Mock Server/Client 与运行说明见 [`testdata/`](testdata/README.md)。测试工具使用 `uv + Python` 维护，不读取 OpenBridge 配置，也不持有真实上游 credential。

## 推荐阅读顺序

| 文档 | 内容 | 分类 |
|---|---|---|
| [文档总索引](docs/README.md) | 四类功能文档的统一入口 | 项目级入口 |
| [功能需求](docs/functional-requirements/README.md) | 产品范围、网关 API/兼容、配置与凭证、路由韧性、观测与交付证据 | 功能需求 |
| [实施现状](docs/implementation-status/README.md) | 当前代码已证明行为、能力探测与验证记录 | 实施现状 |
| [实施计划](docs/implementation-plans/README.md) | 当前焦点及按功能组织的实施方案 | 实施计划 |
| [参考文档](docs/references/README.md) | OpenAI 协议和参考项目事实 | 参考文档 |
| [产品范围](docs/functional-requirements/product-scope.md) | 单用户部署、首要用户结果、边界与非目标 | 功能需求 |
| [网关 API 与客户端兼容](docs/functional-requirements/gateway-api-compatibility.md) | 下游 endpoint、原生 JSON/SSE、tool、continuation 与 Codex 扩展边界 | 功能需求 |
| [Bootstrap、代码注册表、凭证与受信运行边界](docs/functional-requirements/configuration-and-credentials.md) | bootstrap、显式 Provider 注册、secret 与网络信任边界 | 功能需求 |
| [路由与 Provider 韧性](docs/functional-requirements/provider-resilience.md) | alias 候选选择、状态亲和、限流、冷却、重试与错误传播 | 功能需求 |
| [调用统计与可观测性](docs/functional-requirements/observability.md) | usage、TTFT/TTFB、终态错误率和 headless 输出边界 | 功能需求 |
| [当前实现说明](docs/implementation-status/current-implementation.md) | 当前代码真正验证的行为和未证明事项 | 实施现状 |
| [当前开发焦点](docs/implementation-plans/current-focus.md) | 一个短周期行为的测试先行记录 | 实施计划 |
| [服务架构](docs/implementation-plans/service-architecture.md) | 单服务架构、原生/桥接双路径、路由与状态边界 | 实施计划 |
| [参考项目比较矩阵](docs/references/project-comparison.md) | Codex、Hermes、LiteLLM、cc-switch、CLIProxyAPI 的研究职责 | 参考文档 |

文档分类与维护规则见 [`docs/README.md`](docs/README.md)。

## 当前非目标

- 多租户、团队成员、principal/ACL、面向下游用户/key 的配额、计费、合规审计和独立控制面；
- 同 Provider 多账号池、credential 轮换池或账号级负载均衡；
- OpenAI 全部资源 API、Realtime、Files、Conversations 或管理 API；
- 首版 Responses WebSocket transport；Codex 基线使用独立 custom Provider，并显式配置 `supports_websockets = false`；
- 将 Chat ↔ Responses 承诺为无损；不可表达的能力必须拒绝或显式标记；
- 让业务请求动态提供任意上游 URL、认证 header、credential 或转换脚本；
- 让 OpenBridge 执行 Agent 返回的通用 function tool；Protocol Bridge 只转换 wire-level tool call/result。
- GUI、Web 控制台、客户端注册/配置管理或面向用户的管理服务。

## 关键术语

- **Provider Family**：代码中实现的一类协议和认证行为，例如 `openai`、`openai-compatible`、`anthropic`。
- **Deployment**：代码注册表中的一个上游目标，绑定 Provider、base URL、credential binding、上游模型和能力。
- **Public model alias**：客户端使用的稳定模型名，例如 `code-primary`；映射到有序 deployment candidates。
- **RoutePlan / RouteSnapshot**：单次请求固定的 deployment、协议模式、能力判断、credential binding 与 fallback 边界。
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
