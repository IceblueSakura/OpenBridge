# 当前实现说明

## 状态与范围

本文只记录当前可运行入口、外部行为、Provider 注册和验证状态。模块分层、类型职责与内部数据流统一见
[当前代码架构](current-architecture.md)。OpenBridge 仍是实验性原型；最近一次记录已通过全量 Rust 测试与
Clippy，但不代表真实 Provider、外部 SDK、负载或长期运行验收。

## 当前运行入口

默认启动：

```bash
cp .env.example .env
cp config/users.example.toml config/users.toml
# 编辑 users.toml 中的用户/API Key，并在 .env 中填写实际使用 Provider 的 API key。
cargo run --bin openbridge --locked
```

`bootstrap.toml` 包含 loopback listener、私有用户文件位置、request/SSE 上限和共享 HTTP client 参数。Provider、Model、
Upstream Target、Upstream API、Route、Public Model、endpoint 和 credential binding 均由 Rust 代码注册；
修改后需要重新编译或重启。

| Endpoint | 当前行为 | 认证 |
|---|---|---|
| `GET /healthz` | 返回 `status` 与 `registry_version` | 无 |
| `GET /v1/models` | 返回代码注册的 Public Model | 静态 Bearer |
| `POST /v1/chat/completions` | OpenAI-compatible Chat Native JSON/SSE | 静态 Bearer |
| `POST /v1/responses` | OpenAI-compatible Responses Native JSON/SSE | 静态 Bearer |

下游用户和 API Key 来自启动时读取的私有 `config/users.toml`。OpenAI API key 来自 `OPENAI_API_KEY`，
LongCat API key 来自 `LONGCAT_API_KEY`。服务与 probe 可选加载 `.env`，已有进程环境变量优先；上游注册表只保存环境变量名称。

## Provider 与请求行为

当前注册 `ProviderKind::OpenAi` 与 `ProviderKind::LongCat`。两者都使用 API-key credential 和
OpenAI-compatible Chat/Responses wire，但分别拥有独立 adapter、endpoint profile、upstream model、能力和
错误分类。这只能证明两个闭合 Provider Family 的当前 Native Path，不能证明异构协议适配。

请求路径当前会：

- 在 egress 前校验 Public Model、协议、streaming、tools、image、structured output、store、continuation、background、输出限制和 reasoning；
- 将 selected Upstream API 的 `upstream_model` 写入请求；
- 保留同协议下未知但合法的 JSON 字段；
- 对 `previous_response_id` 关闭跨 target fallback；
- 保持非流式 status/body 和有限安全 header；
- 保持流式原始 bytes，同时检查 UTF-8、SSE framing、event size 与 terminal；
- 仅在流式请求首个业务输出前执行有限 retry/fallback；输出后不拼接其他响应；
- 在下游丢弃 body 时取消相应上游 stream。
- 认证后将稳定用户身份写入请求上下文，并记录不含 API Key/正文的结构化 response-start 日志。

## 显式 probe

`openbridge-probe --target <id>` 复用同一 bootstrap、注册表、credential、adapter 与 transport，可以观察模型
列表、最小 Chat/Responses 请求和 function call/result replay。它不接受 endpoint、model、header 或
credential 覆盖，不修改注册表，也不自动改变 capability。

## 验证状态

仓库中的 Rust 测试源码覆盖 bootstrap/registry 校验、模型规则、reasoning gate、认证、Provider model 改写、
capability routing、`/v1/models`、首输出前 fallback、retry header、SSE terminal、partial failure、取消和 probe。
`tests/sdk_compatibility.rs` 是 ignored integration test，需要外部 Python/Node SDK。

最近一次执行：

```text
cargo fmt --all
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

结果为 54 个测试通过、1 个需要下载 OpenAI Python/Node SDK 的集成测试 ignored，Clippy 零告警。
没有运行外部 SDK、真实 Provider、负载或长期验证。

## 当前未实现

- Chat ↔ Responses Protocol Bridge 和异构协议 Provider；
- Responses WebSocket、Realtime、Files、Conversations 等资源 API；
- OAuth、keyring、私有 secret 文件和多 credential pool；
- 独立 `AttemptManager`、跨请求 cooldown 和动态 health/weight；
- 调用统计与指标导出；
- hosted tool、MCP Tool Bridge 或非 loopback 部署。

## 相关资源

- [当前代码架构](current-architecture.md)
- [能力探测](capability-probing.md)
- [协议测试语料与工具](protocol-test-corpus.md)
- [配置、凭证与受信边界](../functional-requirements/configuration-and-credentials.md)
- [路由与 Provider 韧性](../functional-requirements/provider-resilience.md)
