# 遥测指标

## 状态与范围

本文记录当前 checkout 已实现的进程内遥测口径、采集边界和验证证据。它描述观测事实，不表示真实 Provider 性能、外部
exporter、负载能力或动态选路已经验收。

当前遥测分为两层：

- `GatewayMetricsSnapshot`：不带维度的进程级请求、attempt、韧性和 token 累计值；
- `ProviderMetricSnapshot`：按编译期 Provider attempt 维度聚合的性能、usage 和 cache 快照。

实现门面是 [`src/observability.rs`](../../src/observability.rs)，具体代码位于
[`src/observability/provider.rs`](../../src/observability/provider.rs)、
[`src/observability/request.rs`](../../src/observability/request.rs) 和
[`src/observability/usage.rs`](../../src/observability/usage.rs)。

## Provider 维度

每个已收口的实际 upstream attempt 绑定以下非敏感、编译期维度：

| 字段                 | 含义                                                                  |
|----------------------|-----------------------------------------------------------------------|
| `provider`           | `openai`、`longcat`、`deepseek`、`mimo`、`openrouter` 或 `chatgpt`    |
| `route_id`           | 编译期 Route 标识                                                     |
| `upstream_target`    | 编译期 Upstream Target 标识                                           |
| `upstream_operation` | `chat_completions`、`responses` 或 `embeddings_create` 的上游 operation |
| `public_model`       | 下游请求使用的 Public Model                                           |
| `operation`          | 下游 `chat_completions`、`responses` 或 `embeddings_create` operation |
| `route_mode`         | `native` 或 `bridged`                                                 |
| `streaming`          | 是否为 streaming 请求                                                 |

指标 key 不包含 request id、user id、credential member、endpoint URL、Authorization、请求正文或响应正文。
这些维度来自已校验的静态注册表，不允许业务请求动态创建。

## Attempt 结果指标

`ProviderMetricSnapshot` 对每个维度累计：

| 指标                        | 口径                                                             |
|-----------------------------|------------------------------------------------------------------|
| `attempts_started`          | 已完成收口的实际上游 attempt 数；未结束 attempt 尚未出现在快照中 |
| `attempts_completed`        | 原始上游 body 正常到达 EOF                                       |
| `attempts_http_failed`      | 上游返回非 2xx status；在 headers 边界收口                       |
| `attempts_transport_failed` | 没有取得 HTTP response 的 transport failure                      |
| `attempts_stream_failed`    | body、SSE framing 或协议 terminal 失败                           |
| `attempts_cancelled`        | 上游 body 完成前发生取消                                         |

retry 和 fallback 仍由全局 `GatewayMetricsSnapshot` 记录；Provider 快照将每个实际 attempt 单独归类， 因此一次 retry 会增加
attempt 数，但不会把两个 Provider attempt 合并成一次请求。

## 性能指标

所有时间指标都使用 `count`、`sum_ms`、`min_ms` 和 `max_ms` 聚合：

- `response_ready_ms`：从该 Provider attempt 开始到收到上游 response headers；
- `upstream_first_byte_ms`：从 attempt 开始到第一个非空原始上游 body chunk；
- `upstream_ttft_ms`：从 attempt 开始到原始 SSE 中第一个 text/tool 业务增量；
- `gateway_ttft_ms`：从下游请求开始到网关观察到第一个下游 text/tool 业务增量；
- `duration_ms`：原始 upstream body 生命周期；如果尚未观察到 EOF，则在 error/cancel 边界收口；
- `generation_duration_ms`：`upstream_ttft_ms` 到原始 upstream body 完成之间的时间。

非流式请求没有 `upstream_ttft_ms`。Embeddings 不产生上下游 TTFT 或 generation duration 样本。没有明确 output
token，或生成时长为零时，不产生速度观测。

`output_speed` 使用定点整数表示：`milli_tokens_per_second = tokens_per_second × 1000`，避免在 原子累计中使用浮点数。速度由
Provider 明确返回的 output token 和 generation duration 计算，不能由 SSE chunk 数或字节数推算。

## Token 与 cache usage

当前对 Chat/Responses 的明确 usage 做统一归一化：

- 输入 token：`input_tokens` 或 `prompt_tokens`；
- 输出 token：`output_tokens` 或 `completion_tokens`；
- 总 token：`total_tokens`，缺失时仅在 input/output 都明确时相加；
- cache read：`cached_input_tokens`、`cache_read_input_tokens`、`cached_tokens`，以及
  `prompt_tokens_details.cached_tokens` 或 `input_tokens_details.cached_tokens`；
- cache write：`cache_write_input_tokens`、`cache_creation_input_tokens`，以及对应 details 字段。

Embeddings 只把 `usage.prompt_tokens` 记录为 input tokens、把 `usage.total_tokens` 记录为 total tokens；不产生
output-token observation、generation duration 或 output speed。原始文本、token array、`user`、float vector、 base64 和完整
body 不进入 tracing event、metrics label 或 snapshot。

`usage_observations` 只统计至少解析到一个 usage 字段的 attempt；`input_token_observations`、
`output_token_observations` 和 `total_token_observations` 分别是对应 token 字段实际出现的次数。每请求 平均 token
可用对应累计值除以对应 observation 数计算，缺失字段不会被当成零。
`cache_observations` 只统计明确出现 cache read/write 字段的 attempt；`cache_read_observations` 只统计 明确出现 cache read
字段的 attempt。缓存命中率的口径是
`cache_hit_requests / cache_read_observations`；分母为零时命中率未知，没有 Provider cache 字段的请求 不会被误记为 cache
miss。`cache_hit_requests` 只在明确的 cache read token 大于零时增加。

generation JSON usage capture 与 SSE event 分别使用 JSON response/event 上限；超限、缺失或无法解析时不估算 token 或 cache
usage。Embeddings 成功体由 endpoint validator 在同一 JSON response budget 内先完整验证，再记录 明确 usage；非法成功体不提交下游。

## 读取方式

嵌入式调用方可通过：

```rust
let snapshots = state.metrics().provider_snapshots();
```

`GatewayState::metrics()` 返回共享的 `GatewayMetrics` 句柄，快照按 Provider 维度排序。当前运行二进制 尚未提供 `/metrics`
HTTP API、Prometheus exporter、OpenTelemetry exporter、持久化或跨进程聚合。 Provider attempt 还会输出脱敏的
`provider_attempt_completed` tracing event，方便在没有 exporter 时收集日志。

## 与请求生命周期的关系

请求仍由 `downstream_request_completed` 负责提交唯一的下游终态。Provider 快照与下游终态是不同口径：

- Provider 可以成功返回 body，但网关桥接/下游消费随后失败；
- Provider HTTP failure 可能触发 retry/fallback，最终请求仍可能成功；
- 下游取消只有在原始 upstream body 尚未完成时才计入 Provider `attempts_cancelled`；
- `gateway_ttft_ms` 包含路由、transport、bridge 和网关输出路径，不能直接当作 Provider 内部生成 TTFT。

对于已知长度的 JSON body，底层 `HttpBody` 可以在返回最后一个 data/trailer frame 后立即声明 end-stream，Hyper 不保证再 poll
一次独立 EOF。Provider 与下游两层 observer 都在该最后 frame 上提交完成； 只有底层尚未结束时发生的 Drop 才归类为取消。该语义保留原始
size hint，同时避免把已经完整发送的
`/v1/models`、Native JSON 或非流式 Bridge 响应误记为 `cancelled`。

本次修复只统一 body observer 的终态提交时机，不根据这些指标重排 Route candidate，也不改变 capability gate、state
affinity、retry/fallback、cooldown 或首个下游输出后的提交边界。

## 当前验证证据

当前确定性测试覆盖：

- JSON usage、cached input token 与 Provider 维度快照；
- streaming upstream TTFT 与 gateway TTFT 的分离；
- retry 后的 HTTP failure/完成 attempt 归类；
- response body 取消、pending send 取消、SSE EOF-before-terminal 和 failed terminal。
- 真实 Axum/Hyper loopback 下的已知长度模型列表，以及嵌套 Provider/downstream observer 的 Native JSON 完成终态。
- Embeddings 的 `operation=embeddings_create`、input/total usage、无 output/throughput，以及正文、token、`user`、
  vector/base64 哨兵不进入导出结果；replay 超限只记录一次 attempt。

测试通过 fake upstream transport 隔离 Provider 网络依赖，并通过真实 Axum/Hyper loopback 覆盖下游 HTTP transport；它们只证明
OpenBridge 进程内采集和本地传输边界，不证明真实 Provider 的延迟、token 计数、 cache 语义、负载表现或长期运行结果。

最近一次聚焦验证：`cargo test --locked --test observability_contract` 的 13 个观测契约测试通过；完整
`cargo test --locked`、`cargo fmt -- --check`、`cargo clippy --locked -- -D warnings` 和
`git diff --check` 均通过。`tests/sdk_compatibility.rs` 仍按仓库约定保持 ignored，未运行外部 SDK、 真实 Provider、负载或长期运行验收。
