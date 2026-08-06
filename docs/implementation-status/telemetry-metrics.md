# 遥测指标

## 状态与范围

本文记录当前 checkout 已实现的进程内指标口径、可选 OTLP trace 边界和验证证据。它描述观测事实，不表示真实 Provider 性能、
外部 backend、负载能力或动态选路已经验收。

当前遥测分为三层：

- `GatewayMetricsSnapshot`：不带维度的进程级请求、attempt、韧性和 token 累计值；
- `ProviderMetricSnapshot`：按编译期 Provider attempt 维度聚合的性能、usage 和 cache 快照；
- 可选 OTLP traces：一个 `downstream_request` root 与每个实际出站的 `provider_attempt` child，只通过 startup-only
  OTLP/HTTP exporter 发送；collector host 由 bootstrap 配置所有者选择。

实现门面是 [`src/observability.rs`](../../src/observability.rs)，具体代码位于
[`src/observability/provider.rs`](../../src/observability/provider.rs)、
[`src/observability/request.rs`](../../src/observability/request.rs) 和
[`src/observability/usage.rs`](../../src/observability/usage.rs)；exporter 生命周期位于
[`src/observability/otlp.rs`](../../src/observability/otlp.rs)。

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

现有维度没有合并：Provider family 用于横向比较，target/route 用于定位实例与 fallback，upstream/downstream operation 和
route mode 用于区分 Native/Bridge，Public Model 保留客户端契约身份，streaming 则隔离 JSON 与 SSE 延迟分布。它们都是编译期
低基数值；删除任一项都会使上游速度或稳定性问题失去归属。

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

无需增加会与计数器重复的 `error_rate` 字段。对一个已完成快照可按以下口径派生：

- 下游错误率：`(requests_http_failed + requests_failed) / (requests_completed + requests_http_failed + requests_failed)`；
- 下游取消率：`requests_cancelled / (requests_completed + requests_http_failed + requests_failed + requests_cancelled)`；
- Provider attempt 错误率：`(attempts_http_failed + attempts_transport_failed + attempts_stream_failed) /
  (attempts_started - attempts_cancelled)`；
- retry/fallback 压力分别用 `upstream_retries / upstream_attempts` 与 `route_fallbacks / upstream_attempts` 观察。

分母为零时结果未知；取消与 Provider error 分开，避免把客户端主动中断误判为上游不稳定。`requests_started` 包含尚未收口请求，
需要终态错误率时应使用上面的终态分母。

## 性能指标

所有时间指标都使用 `count`、`sum_ms`、`min_ms` 和 `max_ms` 聚合：

- `response_ready_ms`：从该 Provider attempt 开始到收到上游 response headers；
- `upstream_first_byte_ms`：从 attempt 开始到第一个非空原始上游 body chunk；
- `upstream_ttft_ms`：从 attempt 开始到原始 SSE 中第一个非空 text/tool/reasoning token delta；
- `gateway_ttft_ms`：流式 Chat/Responses 从下游请求开始到第一个下游 text/tool/reasoning token delta；成功的非流式
  Chat/Responses 则到第一个非空下游 JSON body chunk，即客户端首次得到完整响应 JSON 的可观测时刻；
- `duration_ms`：原始 upstream body 生命周期；如果尚未观察到 EOF，则在 error/cancel 边界收口；
- `generation_duration_ms`：`upstream_ttft_ms` 到原始 upstream body 完成之间的时间。

非流式请求没有 `upstream_ttft_ms`、`generation_duration_ms` 或 `output_speed`：单个完整 JSON response 不暴露 Provider
实际生成首 token 的时刻，不能用总响应时间伪造解码窗口。Embeddings 不产生上下游 TTFT 或 generation duration 样本。没有明确
output token，或生成时长为零时，不产生速度观测。

`output_speed` 使用定点整数表示：`milli_tokens_per_second = tokens_per_second × 1000`，避免在原子累计中使用浮点数。速度由
Provider 明确返回的 output token 和 generation duration 计算，不能由 SSE chunk 数或字节数推算。reasoning delta 是生成窗口
起点，但不改变 Public Model reasoning capability；这避免 total output tokens 包含 reasoning 时只用可见文本阶段作分母。
平均值按 `sum_milli_tokens_per_second / count / 1000` 计算；时间平均值按 `sum_ms / count` 计算，min/max 用于观察离散度。

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
usage。Chat/Responses usage 只从原始 upstream observer 提交；下游 JSON 不再重复缓存和解析。成功的非流式 Chat/Responses 只在首个
非空 JSON chunk 触发一次 gateway TTFT 原子门控，原始 upstream observer 已明确分类为 `failed`/`incomplete` 的 terminal 不产生该样本；下游 SSE 只在 TTFT 尚未知时解析完整 event，命中首个 token-bearing delta 后停止该观测热路径。Embeddings 成功体由 endpoint validator 在同一 JSON response
budget 内先完整验证，再记录明确 usage；非法成功体不提交下游。

## 读取方式

嵌入式调用方可通过：

```rust
let snapshots = state.metrics().provider_snapshots();
```

`GatewayState::metrics()` 返回共享的 `GatewayMetrics` 句柄，快照按 Provider 维度排序。当前运行二进制提供受 Bearer
保护的 `/openbridge/v1/metrics` 与 `/openbridge/v1/metrics/providers` JSON 读取接口。默认不产生 OTLP egress；显式配置
`[telemetry.traces].otlp_http_endpoint` 后，运行二进制使用固定 protobuf `/v1/traces`、空 exporter headers、有界 batch queue、
500 ms export timeout 与有界 shutdown。尚未提供 OTLP metrics、OTLP logs、Prometheus exporter、持久化或跨进程聚合。
Provider attempt 仍输出脱敏的 `provider_attempt_completed` 本地 tracing event；OTLP layer 不导出 tracing events。

两个 metrics endpoint 仍执行静态 Bearer 认证，但不创建 `RequestObservation`，因此读取前后 gateway/provider 快照保持不变。

## 与请求生命周期的关系

请求仍由 `downstream_request_completed` 负责提交唯一的下游终态。启用 trace 时，同一生命周期结束一个
`downstream_request` root，并让每个实际 Provider attempt 在原有唯一 terminal 边界结束对应 child span。Provider 快照与下游终态是不同口径：

- Provider 可以成功返回 body，但网关桥接/下游消费随后失败；
- Provider HTTP failure 可能触发 retry/fallback，最终请求仍可能成功；
- 下游取消只有在原始 upstream body 尚未完成时才计入 Provider `attempts_cancelled`；
- `gateway_ttft_ms` 包含路由、transport、bridge 和网关输出路径；非流式样本还表示完整 JSON 首次可见，不能直接当作
  Provider 内部生成 TTFT。

首个 downstream body byte、每个 attempt 的 upstream body byte，以及 upstream/gateway TTFT 都使用一次性原子门控；后续
chunk/delta 不再重复获取请求或 Provider 状态锁。原始 upstream SSE 仍须完整解析 terminal/usage，下游 SSE 在 TTFT 前须单独解析，
因为 Bridge 或输出调度可能使 gateway TTFT 不等于 upstream TTFT；这部分不是可删除的重复观测。

对于已知长度的 JSON body，底层 `HttpBody` 可以在返回最后一个 data/trailer frame 后立即声明 end-stream，Hyper 不保证再 poll
一次独立 EOF。Provider 与下游两层 observer 都在该最后 frame 上提交完成； 只有底层尚未结束时发生的 Drop 才归类为取消。该语义保留原始
size hint，同时避免把已经完整发送的
`/v1/models`、Native JSON 或非流式 Bridge 响应误记为 `cancelled`。

这些观测修复只改变 TTFT/生成窗口、usage 采集所有权和 metrics 读取副作用，不根据指标重排 Route candidate，也不改变
capability gate、state affinity、retry/fallback、cooldown 或首个下游输出后的提交边界。

## 当前验证证据

当前确定性测试覆盖：

- JSON usage、cached input token、非流式 generation gateway TTFT 与 Provider 维度快照；
- streaming upstream TTFT 与 gateway TTFT 的分离；
- retry 后的 HTTP failure/完成 attempt 归类；
- response body 取消、pending send 取消、SSE EOF-before-terminal 和 failed terminal。
- 真实 Axum/Hyper loopback 下的已知长度模型列表，以及嵌套 Provider/downstream observer 的 Native JSON 完成终态。
- Embeddings 的 `operation=embeddings_create`、input/total usage、无 output/throughput，以及正文、token、`user`、
  vector/base64 哨兵不进入导出结果；replay 超限只记录一次 attempt。
- 受 Bearer 保护的进程级和 Provider 快照 HTTP endpoint 返回现有结构，认证失败在 handler 前结束，响应不包含 downstream token。
- metrics endpoint 连续读取不改变 gateway/provider 快照，reasoning-only Chat stream 也产生 upstream/gateway TTFT。
- bootstrap 配置 contract 接受 loopback、非 loopback IP 与 DNS collector host，并继续拒绝 HTTPS、缺失 host、URL credential、
  自定义 path/query/fragment 和 exporter header；该测试不产生真实远程 egress。
- loopback fake collector 解码 OTLP protobuf 后只看到一个 request root 与一个 attempt child；parent/child、稳定字段、resource
  identity、精确 attribute allowlist、无 exporter Authorization header 及敏感字节缺失均由 contract test 断言。
- exporter 未配置时 fake collector 收到零请求；collector 阻塞超过 export timeout 时，业务 status/body/metrics 保持一致，请求与
  exporter shutdown 都在独立的有界 timeout 内结束。

测试通过 fake upstream transport 隔离 Provider 网络依赖，并通过真实 Axum/Hyper loopback 覆盖下游 HTTP transport；它们只证明
OpenBridge 进程内采集和本地传输边界，不证明真实 Provider 的延迟、token 计数、 cache 语义、负载表现或长期运行结果。

2026-08-06 的修复前真实 MiMo 证据：Chat streaming 返回 64 output tokens，但 reasoning-only wire 没有 TTFT/速度样本；
Responses 把 510 output tokens 除以首个可见文本后的 605 ms，得到约 842.975 tokens/s。脱敏事件形状分别是 Chat
`delta.reasoning_content` 与 Responses `response.reasoning_text.delta`，usage 明确返回 reasoning token detail。

最终代码的独立构建使用同一私有配置和 `mimo-v2.5` 复测，Chat/Responses streaming 均返回 HTTP 200 并正常 terminal；gateway
快照为 2 started、2 completed、0 error/cancel，连续读取 gateway/provider endpoint 前后完全一致。Chat 记录 upstream/gateway
TTFT 1,618 ms、64 output tokens、2,509 ms generation duration 和约 25.508 tokens/s；Responses 记录 upstream/gateway TTFT
747/748 ms、173 output tokens、6,250 ms generation duration 和约 27.680 tokens/s。该单次真实请求只证明本次 wire 与计算边界，
不代表负载、长期分位数或 Provider SLA。

同日的非流式修复复测使用 `stream:false` 调用同一 `mimo-v2.5`：Chat/Responses 均返回 HTTP 200 JSON，客户端总耗时分别为
2,147 ms 与 967 ms，Provider gateway TTFT 分别为 2,098 ms 与 962 ms。两个 snapshot 的 upstream TTFT、generation duration
和 output speed 都保持 0 样本，usage 与 token 正常累计；gateway 为 2 started、2 completed、0 error，连续读取无副作用。
这些 gateway TTFT 只证明客户端首次获得完整 JSON 的时间，不证明 Provider 实际首 token 或解码速度。

最近一次记录的验证：bootstrap 配置 contract 与 OTLP trace contract 通过；Rust 测试与 Clippy 的具体命令、版本和未执行验收层以
[实施现状目录](README.md)及相关专题页为准。未运行真实 Provider、外部 SDK、负载或长期运行验收。
