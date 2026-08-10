# OpenTelemetry 遥测

## 状态与范围

当前 checkout 使用 OpenTelemetry Rust 0.32 的 traces 与 metrics signal。两者都默认禁用，只有 bootstrap 中显式配置对应
collector base 后才创建 exporter：

```toml
[telemetry.traces]
otlp_http_endpoint = "http://127.0.0.1:4318"

[telemetry.metrics]
otlp_http_endpoint = "http://127.0.0.1:4318"
```

实现门面是 [`src/observability.rs`](../../src/observability.rs)。[`src/observability/otlp.rs`](../../src/observability/otlp.rs)
拥有 `TelemetryRuntime`、共享 resource、OTLP/HTTP exporter、reader/processor 和 shutdown；
[`src/observability/metrics.rs`](../../src/observability/metrics.rs) 只定义固定 instruments；
[`src/observability/provider.rs`](../../src/observability/provider.rs)、
[`request.rs`](../../src/observability/request.rs) 与 [`usage.rs`](../../src/observability/usage.rs) 保留协议生命周期观测；
[`http_logging.rs`](../../src/observability/http_logging.rs) 只渲染 bootstrap 显式启用的本地下游 HTTP snapshot。

OpenBridge 不再维护原子计数快照、Provider `BTreeMap`、自算 sum/min/max 聚合或 JSON metrics handler。
`GET /openbridge/v1/metrics` 与 `GET /openbridge/v1/metrics/providers` 已删除，OpenAPI 也不再声明这些路径和 snapshot schema。

## Exporter 生命周期

traces 与 metrics 共享：

- `service.name = "openbridge"`；
- 每次进程启动唯一、非敏感的 `service.instance.id`；
- bootstrap 所有者提供的无 credential `http` collector base；
- 固定 protobuf signal path：`/v1/traces` 或 `/v1/metrics`；
- 500 ms 单次 HTTP timeout、禁止 redirect、协议自带 header 白名单；
- 业务请求不能选择 endpoint、protocol、header、resource 或采集策略。

HTTP client 在发送前只保留 `Content-Type`、可选 `Content-Encoding` 和 SDK `User-Agent`；环境注入的 Authorization 或租户
header 会被删除。collector 不可用、超时或背压不进入业务请求路径。关闭时 metrics provider 与 tracer provider 在 blocking
worker 中执行 flush/shutdown，并受外层有界 timeout 约束。

metrics 使用：

- `PeriodicReader`，固定 60 秒采集间隔；
- `MetricExporter`，固定 cumulative temporality；
- SDK 原生 monotonic sum 与 explicit-bucket histogram 聚合；
- 每个 instrument 最多 1,024 个 attribute set，超出时由 SDK overflow 聚合；
- shutdown 时最终 collection/flush。

未配置 `[telemetry.metrics]` 时使用 OpenTelemetry no-op meter，不创建 reader、exporter worker 或 metrics egress。

## 指标目录

### 请求与韧性

| Instrument | 类型 / 单位 | 口径与 attributes |
|---|---|---|
| `openbridge.downstream.request.started` | Counter / `{request}` | 已认证并进入观察生命周期的请求数，无 attributes。 |
| `openbridge.downstream.request.completed` | Counter / `{request}` | 唯一请求终态；`openbridge.request.outcome` 为 `completed`、`http_failed`、`failed` 或 `cancelled`。 |
| `openbridge.downstream.request.duration` | Histogram / `s` | 完整下游 body 生命周期；带 outcome、downstream operation、Public Model 和 streaming。 |
| `openbridge.routing.events` | Counter / `{event}` | `openbridge.routing.event` 为 `retry`、`credential_rotation`、`route_fallback` 或 `cooldown_skip`。 |

### Provider attempt

| Instrument | 类型 / 单位 | 口径 |
|---|---|---|
| `openbridge.provider.attempt.started` | Counter / `{attempt}` | 实际发起的 Provider call。 |
| `openbridge.provider.attempt.completed` | Counter / `{attempt}` | 唯一 attempt 终态；outcome 为 `completed`、`http_failed`、`transport_failed`、`stream_failed` 或 `cancelled`。 |
| `gen_ai.client.operation.duration` | Histogram / `s` | attempt 开始到 raw upstream body EOF/error/cancel；失败时带低基数 `error.type`。 |
| `openbridge.provider.response_ready.duration` | Histogram / `s` | attempt 到 upstream response headers。 |
| `openbridge.provider.first_byte.duration` | Histogram / `s` | attempt 到首个非空 raw upstream body frame。 |
| `openbridge.provider.time_to_first_token` | Histogram / `s` | streaming raw upstream 中首个 token-bearing text/tool/reasoning delta。 |
| `openbridge.gateway.time_to_first_output` | Histogram / `s` | downstream 首个 generation delta；非流式 Chat/Responses 是完整 JSON 首次可见，不冒充 upstream TTFT。 |
| `openbridge.provider.generation.duration` | Histogram / `s` | upstream TTFT 到 raw upstream EOF；仅在两个边界都明确时记录。 |
| `openbridge.provider.output.speed` | Histogram / `{token}/s` | 明确 output token 除以明确 generation duration；缺失值或零时长不记录。 |

Provider-scoped point 使用以下受信、低基数 attributes：

- `gen_ai.provider.name`、`gen_ai.operation.name`、`gen_ai.request.model`、`gen_ai.request.stream`；
- `openbridge.provider.name`、`openbridge.route.id`、`openbridge.upstream.target`；
- `openbridge.upstream.operation`、`openbridge.downstream.operation`；
- `openbridge.public_model`、`openbridge.route.mode`；
- terminal instrument 额外使用 `openbridge.attempt.outcome`，失败 duration 可使用 `error.type`。

其中 `gen_ai.operation.name` 将 Chat Completions 与 Responses 统一为 `chat`，Embeddings 为 `embeddings`；具体上下游协议仍由
`openbridge.*.operation` 区分。ChatGPT subscription adapter 的 OpenBridge Provider 名为 `chatgpt`，标准 GenAI provider namespace
使用 `openai`。

### Token 与 cache

| Instrument | 类型 / 单位 | 口径 |
|---|---|---|
| `gen_ai.client.token.usage` | Histogram / `{token}` | 只记录 Provider 明确返回的 input/output；`gen_ai.token.type` 为 `input` 或 `output`。 |
| `openbridge.provider.cache.read.token.usage` | Histogram / `{token}` | 明确 cache-read input tokens，包括零值。 |
| `openbridge.provider.cache.write.token.usage` | Histogram / `{token}` | 明确 cache-write input tokens，包括零值。 |
| `openbridge.provider.cache.requests` | Counter / `{request}` | 只有明确 cache-read 字段才记录；`openbridge.cache.result` 为 `hit` 或 `miss`。 |

`gen_ai.client.token.usage` 使用 GenAI semantic conventions 建议的 token buckets：
`1, 4, 16, 64, 256, 1024, 4096, 16384, 65536, 262144, 1048576, 4194304, 16777216, 67108864`。
OpenBridge 不另发 total token：backend 可在相同 attribute 维度合并 input/output，且不会把缺失 usage 当成零。

Chat/Responses usage 支持 `input_tokens`/`prompt_tokens`、`output_tokens`/`completion_tokens` 和明确 cache detail；Embeddings
只产生 input token point，不产生 output、TTFT、generation duration 或 output speed。JSON/SSE 超限、缺失或无法解析时不估算。

## 安全边界

任何 metric point 都不包含 request/trace ID、用户、credential pool/member、Authorization、collector/upstream endpoint URL、
HTTP path/query、原始错误文本、request/response body、tool arguments/result、reasoning 正文、embedding input/vector/base64。
属性值来自已验证的静态 registry、typed operation 或固定 outcome vocabulary；业务正文不能动态创建 attribute key/value。

OTLP trace 继续只导出 `downstream_request` root 与每个实际 `provider_attempt` child 的 allowlist attributes，不导出 tracing
events。Provider/request 本地 completion event 仍用于本机诊断，但不作为第二套 metrics 聚合。随附开发配置显式全开的四个
`[logging]` 开关所产生的认证后 downstream header/body 本地事件同样不会进入 OTLP；缺表/缺字段时对应开关回退关闭，其 header 强制脱敏，body capture 受现有 request/JSON
response budget 约束，并在长流、错误或取消时显式标记不完整或截断。

## 生命周期语义

- 每个请求和 attempt 只提交一个 terminal；retry/fallback 会产生新的 attempt，而不是覆盖前一条。
- Provider HTTP/transport/body/SSE failure 与 downstream cancel 分开计数。
- 已知长度 body 可在最后 data frame 同时到达 EOF；两层 observer 在该 frame 收口，只有底层未结束时 Drop 才算取消。
- streaming TTFT 只由 token-bearing text/tool/reasoning delta 触发一次；metadata、空 delta 和 `[DONE]` 不触发。
- 非流式完整 JSON 没有 upstream TTFT/generation speed 样本；gateway 首次输出只表示客户端可见响应时间。
- 指标不参与 Route 选择、retry/fallback、cooldown、capability gate 或 state affinity。

## 当前验证证据

2026-08-07，本轮实际运行的配置、Ingress、转发、Observability、OTLP metrics 与 OTLP traces 聚焦测试均通过；
`cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings` 与 `git diff --check` 均通过。

- `tests/config_contract.rs` 验证 traces/metrics collector base 的允许与拒绝 URL shape，以及未知 exporter 配置拒绝。
- `tests/otlp_metrics_contract.rs` 通过真实 loopback collector 解码 OTLP protobuf，验证固定 `/v1/metrics`、resource/scope、
  Counter/Histogram、单位、token buckets、request/attempt/timing/usage/cache attributes、敏感值缺失、disabled 零 egress、
  blocked collector 的业务隔离与有界 shutdown。
- `tests/otlp_trace_contract.rs` 验证固定 `/v1/traces`、request/attempt parent-child、attribute allowlist、敏感值缺失和 collector
  故障隔离。
- `tests/observability_contract.rs` 使用 OpenTelemetry SDK 官方 in-memory exporter 验证 retry、HTTP/stream failure、取消、
  JSON/SSE、reasoning TTFT、Embeddings usage 和已知长度 body terminal；测试层不要求生产 snapshot API。
- `tests/ingress_contract.rs` 验证两个旧 metrics path 返回 `404`，OpenAPI 不含旧 path/schema。

2026-08-10，本地下游 HTTP 内容日志扩展的实际验证通过：`tests/observability_contract.rs` 的 15 个测试确认四个 bootstrap
开关保持独立，并覆盖 header 脱敏、有界正文 snapshot、成功、错误、取消和流式生命周期；`tests/otlp_trace_contract.rs` 的
2 个 loopback 测试确认四个开关全开时，header/body marker 仍不进入 OTLP protobuf。`cargo fmt -- --check`、
`cargo test --locked`、`cargo clippy --locked -- -D warnings` 与 `git diff --check` 均通过。

这些确定性测试和 loopback collector 只证明当前进程的生命周期、SDK 聚合与 OTLP/HTTP protobuf 边界；不证明真实 Provider
指标准确性、外部 collector/backend、dashboard/告警、负载、长期运行或多进程聚合。真实 MiMo 的历史单次 timing 证据不能视为
当前 Provider SLA，本轮未复测真实 Provider。
