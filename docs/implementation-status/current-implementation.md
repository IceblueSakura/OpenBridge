# 当前实现说明

## 状态与范围

本文只描述当前代码和测试已经证明的行为。OpenBridge 仍是实验性原型；异构协议 Provider、Protocol Bridge、Responses WebSocket、真实 OAuth、调用统计和跨请求 cooldown 尚未实现。模块与依赖关系见[当前代码架构](current-architecture.md)。

## 启动与注册表

启动过程：

```text
config/bootstrap.toml
→ config::load_bootstrap
→ providers::compiled_definition
→ registry::build_registry
→ Arc<RegistrySnapshot>
→ HTTP listener
```

`bootstrap.toml` 只包含 loopback listener、request/SSE 上限和共享 HTTP client 策略。
Provider、Model、Deployment、Alias、endpoint、credential binding 和 capability 全部由 Rust 代码注册。

当前没有：

- `config/routes.toml`；
- route TOML schema；
- `OPENBRIDGE_ROUTES_CONFIG`；
- `ConfigManager` 或 `ArcSwap`；
- route 热重载；
- 动态 Provider、model、header 或转换脚本。

注册表构建会验证 ID、引用、credential locator、endpoint/profile、timeout、Provider 能力上界、
模型 token 限制、参数、reasoning、reasoning level、deployment constraint 和 alias candidate。
任何错误都会在监听前失败。

## Provider 实现

当前有 `ProviderKind::OpenAi` 与 `ProviderKind::Meituan`，两者都使用 API-key credential，并都提供 OpenAI-compatible Chat/Responses Native Path。它们是两个闭合 Provider Family，但尚不能证明异构协议适配边界。

通用契约位于：

- `src/provider/contracts.rs`
- `src/provider/credential.rs`
- `src/provider/mod.rs`

具体 descriptor、注册项和 adapter 位于：

- `src/providers/openai.rs`
- `src/providers/meituan.rs`

Provider-independent 模型事实位于：

- `src/models/*`

顶层显式注册入口位于：

- `src/providers/mod.rs`

两个 adapter 分别负责各自的：

- Chat/Responses 相对 path；OpenAI 使用 `/v1/*`，Meituan 使用 `/openai/v1/*`；
- 写入 deployment 的实际 `upstream_model`；
- `Content-Type` 与 Bearer header；
- Chat `[DONE]` 和 Responses terminal event；
- status/error/retry hint 分类；
- `GET /v1/models` discovery request。

Pipeline 不再重写 model，只保留原始请求并完成 capability/candidate 选择。

## 模型与路由

编译期模型定义包含：

- stable logical id、名称和描述；
- 可选 input/output token 上限；
- 支持参数集合；
- reasoning 的 `Supported`、`Unsupported`、`Unknown`；
- `Minimal`、`Low`、`Medium`、`High`、`XHigh` level 集合。

显式 reasoning 请求只有在模型标记支持时才能路由；显式 level 还必须命中模型 level 集合。未知状态、
未知 level 或不支持 level 都会在 egress 前拒绝，不会自动降级。

Alias 保存有序 deployment candidate。请求会按协议、streaming、function tools、parallel tools、
image、structured output、store、continuation、background、输出限制和 reasoning 筛选 candidate。
`previous_response_id` 会关闭跨 deployment fallback。

## 下游 HTTP API

| Endpoint | 当前行为 | 认证 |
|---|---|---|
| `GET /healthz` | 返回 `status` 与 `registry_version` | 无 |
| `GET /v1/models` | 返回代码注册的 public alias | 静态 Bearer |
| `POST /v1/chat/completions` | OpenAI native JSON/SSE 转发 | 静态 Bearer |
| `POST /v1/responses` | OpenAI native JSON/SSE 转发 | 静态 Bearer |

下游 token 来自 `OPENBRIDGE_DOWNSTREAM_TOKEN`；OpenAI API key 来自 `OPENAI_API_KEY`；LongCat API
key 来自 `LONGCAT_API_KEY`。服务与 probe 可选加载 `.env`，已有进程环境变量优先；snapshot 只保存
环境变量名称，不保存值。

默认启动：

```bash
cp .env.example .env
# 编辑 .env，填写下游 token 以及实际使用 Provider 的 API key。
cargo run --bin openbridge --locked
```

## Transport 与 SSE

- 共享 reqwest client；
- endpoint base 只来自代码注册表，必须是安全 HTTPS URL；
- adapter 只能生成相对 URI；
- path prefix 被显式保留；
- redirect 禁用；
- 认证 header 标记 sensitive；
- 非流式响应保留上游 status、body 和有限安全 header；
- 流式响应保持原始 bytes，同时验证 UTF-8、SSE framing、event size 和 terminal；
- 下游丢弃 body 时取消上游 stream；
- 已开始的 stream 不拼接 retry/fallback。

Streaming 请求对 429、5xx、连接错误和 timeout 只在首个下游 body 输出前进行有限 retry，并可进入下一个
兼容 candidate。当前仍是单请求固定次数原型，不包含跨请求 cooldown 和联合重试预算。

## 显式 probe

`openbridge-probe` 使用同一 bootstrap 与代码注册表，可执行：

- 上游模型列表；
- 最小 Chat 请求；
- 最小 Responses 请求；
- 两种协议的 function call/result replay。

CLI 不接受 endpoint、model 或 credential 覆盖，不修改注册表，也不自动改变 capability。

## 验证

默认命令：

```bash
cargo fmt -- --check
cargo test --locked
cargo clippy --locked -- -D warnings
```

测试覆盖：

- bootstrap 与 typed registry 校验；
- deployment constraint 只收窄；
- reasoning level gate；
- endpoint/path prefix 安全；
- 静态下游认证；
- Provider model 改写；
- capability routing；
- `/v1/models`；
- output-before fallback；
- retry header；
- SSE UTF-8/framing/terminal；
- partial stream failure；
- cancellation；
- 显式 probe。

`tests/sdk_compatibility.rs` 是 ignored integration test，会使用运行时 OpenAI Python/Node SDK 验证
Chat/Responses stream/non-stream 和 function-tool 往返。

## 尚未实现

- 异构协议 Provider 与相应协议证据；
- Chat ↔ Responses bridge；
- Responses WebSocket、Realtime、Files、Conversations 等资源 API；
- 真实 OAuth；
- keyring/私有 secret 文件；
- 远程探测证据自动叠加到路由；
- 跨请求 cooldown、动态 health/weight；
- usage、TTFT/TTFB 和终态统计；
- hosted tool 或 MCP Tool Bridge；
- 非 loopback 部署。

## 相关资源

- [代码注册表与路由](../implementation-plans/configuration-and-routing.md)
- [当前代码架构](current-architecture.md)
- [注册表与路由架构迁移计划](../implementation-plans/registry-architecture-migration.md)
- [Provider adapter 与数据流](../implementation-plans/provider-adapters-and-dataflow.md)
- [能力探测](capability-probing.md)
- [交付与证据要求](../functional-requirements/delivery-and-evidence.md)
