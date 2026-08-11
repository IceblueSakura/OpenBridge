# OpenTelemetry 遥测

## 当前行为

OpenBridge 使用 OpenTelemetry Rust traces 与 metrics；Bootstrap 省略对应 signal table 时禁用，只有显式配置 OTLP/HTTP
collector base 才创建 exporter。当前随附的 `config/bootstrap.toml` 与 `config/bootstrap.example.toml` 是开发 profile，二者都显式
启用 traces/metrics 并指向 `http://127.0.0.1:4318`。固定 signal path 为 `/v1/traces` 与 `/v1/metrics`，共享
`service.name=openbridge` 和进程唯一、非敏感 `service.instance.id`。

实现门面为 [`src/observability.rs`](../../src/observability.rs)：

- `observability/request.rs` 拥有 downstream lifecycle；
- `observability/provider.rs` 拥有 attempt observation；
- `observability/usage.rs` 解析明确的协议 usage；
- `observability/metrics.rs` 定义固定 SDK instruments；
- `observability/otlp.rs` 拥有 resource、exporter、reader/processor 与 shutdown；
- `observability/http_logging.rs` 只渲染 Bootstrap 显式启用的本地下游 snapshot。

旧进程内 atomic/BTreeMap snapshot 与 JSON metrics handler 已删除，`GET /openbridge/v1/metrics*` 不存在。Metrics 使用 SDK 原生
cumulative Counter/Histogram、固定周期 reader 与 attribute-set overflow；collector 不可用或背压不进入业务路径，进程关闭时执行
有界 flush/shutdown。

## Instrument 范围

固定 instruments 覆盖：

- downstream request started/completed、duration、time-to-response 与 stream first output；
- Provider attempt started/completed、duration、time-to-headers/first token、generation duration 与 output speed；
- retry、fallback、credential rotation、member/target cooldown；
- input/output/total、cached/cache-write/reasoning token usage 与 cache event。

Attributes 只使用低基数、受信字段，如 operation、protocol、Provider、outcome、retryable、status class 与 terminal。request/user/
credential/endpoint URL、body 和错误正文不进入 metrics。OpenAI GenAI semantic-convention name 与 OpenBridge Provider/operation 维度
分开，避免把 ChatGPT subscription 伪装成 OpenAI API-key Provider。

Output speed 仅在成功且同时具有 output tokens、TTFT 与 terminal 时间时记录，使用 generation duration（TTFT 后至 terminal），
不是 total attempt latency。缺失/非法 usage、取消、失败或零 duration 不猜测数值。

## 安全与本地内容日志

OTLP client 禁止 redirect，限制 timeout，并剥离环境注入的 Authorization/租户 header。四个本地内容日志开关只在认证后观察最终
下游边界，header 强制 redaction，body 有界且每方向至多一个终态 snapshot；不记录每个 SSE chunk，也不进入 span-only OTLP。

## 确定性证据

- `tests/observability_contract.rs`：request/attempt 生命周期、usage、内容开关、body 终态与 redaction。
- `tests/otlp_trace_contract.rs`：span hierarchy、attribute allowlist、内容 snapshot 排除和 shutdown。
- `tests/otlp_metrics_contract.rs`：OTLP request/resource、instrument/aggregation、overflow 与无 credential header。
- forwarding/SSE/Embeddings tests：success/failure/EOF/cancel/retry/fallback 的唯一 observation。

## 未证明范围

确定性 tests 不证明外部 collector/backend、dashboard/告警、生产 sink、计费准确性、Provider SLA、负载、长期运行或多进程聚合。
当前没有 OTLP logs、内置 Prometheus、持久化查询/重置或分布式 metrics state。

## 相关文档

- [Observability 需求](../functional-requirements/observability/README.md)
- [启动配置与凭证](features/startup-configuration-and-credentials.md)
- [当前代码架构](current-architecture.md)
