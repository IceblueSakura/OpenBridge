# OpenTelemetry 遥测

## 当前行为

OpenBridge 使用 OpenTelemetry Rust traces 与 metrics；Bootstrap 省略对应 signal table 时禁用，只有显式配置 OTLP/HTTP
collector base 才创建 exporter。当前随附的 `config/bootstrap.toml` 与 `config/bootstrap.example.toml` 是开发 profile，二者都显式
启用 traces/metrics 并指向 `http://127.0.0.1:4318`。固定 signal path 为 `/v1/traces` 与 `/v1/metrics`，共享
`service.name=openbridge` 和进程唯一、非敏感 `service.instance.id`。

实现门面为 [`src/observability.rs`](../../src/observability.rs)：

- `observability/request.rs` 拥有 downstream lifecycle；
- `observability/classification.rs` 拥有有界 request kind、failure stage、error type 与 next action；
- `observability/provider.rs` 拥有 attempt observation；
- `observability/usage.rs` 解析明确的协议 usage；
- `observability/metrics.rs` 定义固定 SDK instruments；
- `observability/otlp.rs` 拥有 resource、exporter、reader/processor 与 shutdown；
- `observability/http_jsonl/` 使用专用有界 writer 将 Bootstrap 显式启用的本地下游 snapshot 写入按 UTC 日期滚动的 JSONL。

旧进程内 atomic/BTreeMap snapshot 与 JSON metrics handler 已删除，`GET /openbridge/v1/metrics*` 不存在。Metrics 使用 SDK 原生
cumulative Counter/Histogram、固定周期 reader 与 attribute-set overflow；collector 不可用或背压不进入业务路径，进程关闭时执行
有界 flush/shutdown。

## Instrument 范围

固定 instruments 覆盖：

- `openbridge.downstream.request.started` Counter，以及 terminal 唯一写入的
  `openbridge.downstream.request.duration`、`openbridge.downstream.response_ready.duration` 与
  `openbridge.downstream.time_to_first_output` histograms；
- `openbridge.provider.attempt.started` Counter，以及物理 attempt 的
  `openbridge.provider.attempt.duration`、response-ready、first-byte、TTFT、generation duration 与成功 output speed
  histograms；不再把物理 attempt 冒充 `gen_ai.client.operation.duration`；
- `openbridge.routing.events` Counter，使用固定 event/reason；request terminal 另带
  `openbridge.request.recovery = none | retry | credential_rotation | fallback | multiple`；
- `gen_ai.client.token.usage` 的 input/output，以及 cached/cache-write/reasoning-output token histograms 和 cache
  hit/miss Counter。total token 仅保留在明确 trace usage 中，不设置重复 metric；
- `openbridge.images.output.count`、`openbridge.images.output.width`、`openbridge.images.output.height` 只记录已验证
  Images success 的图片数量和像素尺寸；不混入 token usage，也不带 prompt、URL 或 user attribute。

不存在 request/attempt completed、failure 或 active instrument。terminal/failure 由对应 duration histogram 的
count/outcome 得到；当前 cumulative temporality 下，active 由 `started - terminal_duration_count` 得到。

Attributes 只使用低基数、受信字段，如 request kind、operation、protocol、Provider、outcome、status class、规范化
`error.type`、failure stage、retryable、next action 与 recovery。request/user/credential/endpoint URL、body、精确 HTTP status
和错误正文不进入 metrics。OpenAI GenAI semantic-convention name 与 OpenBridge Provider/operation 维度分开，避免把
ChatGPT subscription 伪装成 OpenAI API-key Provider。

Output speed 仅在成功且同时具有 output tokens、TTFT 与 terminal 时间时记录，使用 generation duration（TTFT 后至 terminal），
不是 total attempt latency。缺失/非法 usage、取消、失败或零 duration 不猜测数值。

## Trace 范围

认证后 middleware 按 method 与固定 endpoint 记录 `generation | embeddings | models | mcp` request kind。失败 request/attempt
使用固定 `error.type`、retryable 与实际 next action；request 另带 failure stage，存在 HTTP 响应时 trace 保存精确 status。
`request_id` 保留为 root trace correlation attribute，但不进入任何 metric attribute。
Public Model 只在 registry planning 成功后进入 trace/metrics，未知或无效的请求 `model` 不作为 attribute 导出；Models handler 的
404/4xx 由 request terminal owner 归一为有界 `unknown_model`/`invalid_request`，不导出 path 参数。
retry、credential rotation、fallback 与 cooldown skip 使用四个固定名称、固定低基数字段的 allowlisted routing events；普通
tracing events、正文、header 和原始错误继续排除。Bridge 转换失败归于 request `bridge` 阶段：已观察 upstream EOF 的
Provider attempt 保持 completed，否则因网关终止读取而归为 cancelled，二者都不误记为 Provider stream failure。
Timeout trace 只增加闭合 phase（`response_headers | first_event | event_idle | stream_total | non_stream_total`）、当前 response
是否已 ready/committed 以及 request-relative last-event milliseconds；这些值不进入 metric label，也不保留底层错误字符串或 event 内容。
SSE precommit 的 timeout/body-transport attempt 在未下游 commit 时按实际 retry/fallback action 终结；首个下游可见 event 后的
invalid framing、body error 或 terminal 前 EOF 只终结当前 body/request，不生成第二次 attempt。`upstream_body_transport` 是固定
低基数分类，不包含底层错误字符串。
Bridge precommit 消费的 invisible events 只推进安全 event/byte/time 观察与同一个 renderer state；raw event 立即释放，postcommit
从同一 source remainder 继续，不重复 renderer、usage 或 event observation。
Images success-response owner 只在完整 bounded validation 后记录 count/width/height；overflow、body transport、malformed/
contract mismatch 与 cancel 均不记录 image usage。普通 telemetry 只保留 `images_generations` 与闭合 outcome/error，不含 prompt、URL、body 或 credential。

## 安全与本地内容日志

OTLP client 禁止 redirect，限制 timeout，并剥离环境注入的 Authorization/租户 header。四个本地内容日志开关只在认证后观察最终
下游边界，header 强制 redaction，body 有界且每方向至多一个终态 snapshot；不记录每个 SSE chunk，也不进入 stdout 或 reviewed
OTLP trace layer。

## 确定性证据

- `tests/observability_contract.rs`：request/attempt 生命周期、usage、内容开关、body 终态与 redaction。
- `tests/otlp_trace_contract.rs`：span hierarchy、attribute allowlist、内容 snapshot 排除和 shutdown。
- `tests/otlp_metrics_contract.rs`：OTLP request/resource、instrument/aggregation、overflow 与无 credential header。
- forwarding/SSE/Embeddings tests：success/failure/EOF/cancel/retry/fallback 的唯一 observation。
- observability/streaming 单元测试：timeout phase、commit state、last-event timestamp、首次失败保持和内容排除。

2026-08-24 在当前 checkout 执行：

- `cargo test --locked --test sse_contract`、`forwarding_contract`、`bridge_forwarding_contract`、
  `process_replay_contract`、`observability_contract`、`otlp_metrics_contract` 与 `otlp_trace_contract`：通过；
- `cargo fmt -- --check`：通过；
- `cargo check --locked --all-targets`：通过；
- `cargo test --locked`：通过；
- `cargo clippy --locked -- -D warnings`：通过；
- `git diff --check`：通过。

2026-08-23 在当前 checkout 执行：

- `cargo fmt -- --check`：通过；
- `cargo test --locked`：通过；
- `cargo clippy --locked -- -D warnings`：通过；
- `git diff --check`：通过。

2026-08-18 在当前 checkout 执行：

- `rustfmt --edition 2024 --check $(git diff --name-only -- '*.rs')`：通过；
- `cargo test --locked`：通过；
- `cargo clippy --locked -- -D warnings`：通过；
- `git diff --check`：通过。

全局 `cargo fmt -- --check` 现已通过；此前的两个既存格式例外文件已随 provider/model 重构收口，不再是全局格式例外。

## 未证明范围

确定性 tests 不证明外部 collector/backend、dashboard/告警、生产 sink、计费准确性、Provider SLA、负载、长期运行或多进程聚合。
当前没有 OTLP logs、内置 Prometheus、持久化查询/重置或分布式 metrics state。

## 相关文档

- [Observability 需求](../functional-requirements/observability/README.md)
- [启动配置与凭证](features/startup-configuration-and-credentials.md)
- [当前代码架构](current-architecture.md)
