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

运行配置与模板一一对应：`.env` 使用 `.env.example`，`config/bootstrap.toml` 使用
`config/bootstrap.example.toml`，`config/users.toml` 使用 `config/users.example.toml`。

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
- 经 Provider 的受信 request-header hook 把下游 `User-Agent` 覆盖到上游，同时保持认证、cookie、Host 与 proxy header 隔离；
- 保留同协议下未知但合法的 JSON 字段；
- 对 `previous_response_id` 关闭跨 target fallback；
- 保持非流式 status/body 和有限安全 header；
- 保持流式原始 bytes，同时检查 UTF-8、SSE framing、event size 与 terminal；
- 在 stream/non-stream 提交下游 response 前，对 transient status/transport error 使用请求级最多 6 次、每候选最多 2 次的有限 retry/fallback，并执行 50～500 ms capped exponential backoff；
- 当前候选耗尽后只沿 RoutePlan 进入同一 Public Model 的其他完整候选；全部失败时返回最后一个安全 HTTP 错误或稳定 transport error；
- 对 retryable `429`、暂时性上游故障和 transport failure 记录单进程短时 cooldown；后续无状态请求按
  `quota_scope`/`fault_domain` 跳过已知受限边界，target-bound continuation 则继续尝试原 target；
- 在下游中断 pending send、退避等待或丢弃 response body 时取消相应上游工作，不再启动后续 attempt；
- 认证后将稳定用户身份写入请求上下文，并记录不含 API Key/正文的结构化 response-start 日志。

## Protocol Bridge 状态基础

`src/bridge.rs` 提供彼此独立的 Chat 与 Responses stream 状态机。它们按 wire 顺序固定 response/item/call/index
identity，累计 text 与 function arguments，区分 `output_item.done` 和 response terminal，并在 identity 冲突、
不完整 JSON arguments、重复 terminal 或 EOF-before-terminal 时失败关闭。

该模块由现有 canonical 双向 text/parallel-tool/incomplete-arguments SSE fixture 回放验证，但尚未接入
`RouteMode::Bridged`、请求转换、目标 wire renderer 或生产 ingress，因此不构成 Protocol Bridge 已可用的声明。

## 显式 probe

`openbridge-probe --target <id>` 复用同一 bootstrap、注册表、credential、adapter 与 transport，可以观察模型
列表、最小 Chat/Responses 请求和 function call/result replay。它不接受 endpoint、model、header 或
credential 覆盖，不修改注册表，也不自动改变 capability。

## 验证状态

仓库中的 Rust 测试源码覆盖 bootstrap/registry 校验、模型规则、reasoning gate、认证、Provider model 改写、
capability routing、`/v1/models`、stream/non-stream 指数退避、跨 Provider fallback、请求级 attempt 硬上限、
quota/fault scope cooldown、continuation 亲和、retry header、SSE terminal、partial failure、pending
send/backoff/body 取消、canonical bridge fixture replay、真实 loopback HTTP 429 process replay 和 probe。
`tests/sdk_compatibility.rs` 是 ignored integration test，需要外部 Python/Node SDK。日常客户端可见测试优先使用
OpenAI SDK、独立 Python 脚本或 curl，不要求绑定 Codex/Hermes 等 Agent runtime。

2026-08-01 最近一次执行：

```text
cargo fmt -- --check
cargo test --locked
cargo clippy --locked -- -D warnings
git diff --check
```

结果为 72 个测试通过、1 个需要下载 OpenAI Python/Node SDK 的集成测试 ignored，Clippy 零告警，
格式与 diff 检查通过。没有运行外部 SDK、独立 Python/curl 黑盒测试、Codex/Hermes、真实 Provider、
负载或长期验证。

## 当前未实现

- 可执行的 Chat ↔ Responses Bridge Plan、wire renderer、生产 route 和异构协议 Provider；
- Responses WebSocket、Realtime、Files、Conversations 等资源 API；
- OAuth、keyring、私有 secret 文件和多 credential pool；
- 动态 health/weight、持久化或分布式 cooldown 与后台探测；
- 调用统计与指标导出；
- hosted tool、MCP Tool Bridge 或非 loopback 部署。

## 相关资源

- [当前代码架构](current-architecture.md)
- [能力探测](capability-probing.md)
- [协议测试语料与工具](protocol-test-corpus.md)
- [配置、凭证与受信边界](../functional-requirements/configuration-and-credentials.md)
- [路由与 Provider 韧性](../functional-requirements/provider-resilience.md)
