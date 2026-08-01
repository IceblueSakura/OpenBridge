# 调用统计与可观测性需求

## 状态

**Working behavior design。** 本文定义单用户、headless OpenBridge 所需的调用量、usage、首输出时间和终态错误统计。它不要求建立 GUI、独立管理服务、用户计费或合规审计；当前代码尚未实现这些统计，已实现范围以[当前实现说明](../implementation-status/current-implementation.md)为准。

## 1. 目标与边界

服务所有者需要在不记录业务正文的前提下，回答“调用是否成功、慢在哪里、上游报告了多少 token、失败是否集中在某个 Upstream Target/Upstream API 或错误类别”。每个请求的统计必须覆盖原生路径和未来 bridge 路径，且不改变路由、重试、SSE terminal 或对下游的错误传播。

统计属于服务本身的 headless 运维输出，而不是客户端管理面：不提供 GUI、用户列表、下游 key 用量排行、账单结算、配额执行或审计检索。需要这些能力时应另行定义产品边界。

## 2. 统计口径

### 2.1 请求与时延

调用统计的主分母是已通过入站认证、内容类型和大小校验，并进入路由管线的请求。认证失败、非 JSON、过大 body、未知 endpoint 等 ingress 拒绝应单独计数，不能混入上游调用错误率。

| 指标 | 起止点 | 说明 |
|---|---|---|
| `requests_total` | 请求进入路由管线 | 按最终 outcome 计数；一次请求只有一个终态。 |
| `gateway_latency_ms` | 请求进入路由管线 → 终态 | 包括路由、重试、上游等待和已开始 stream 的终结；客户端取消单列。 |
| `gateway_ttft_ms` | 流式请求进入路由管线 → 网关成功写出首个下游 response body byte | 衡量客户端看到首个流式输出的时间。它不是 Provider 声明的“首个文本 token”，不得据此推断模型生成速度。 |
| `gateway_ttfb_ms` | 非流式请求进入路由管线 → 网关成功写出首个下游 response body byte | 非流式请求不伪称 TTFT，应单独报告为 TTFB。 |

已在写出业务 body 后发生的 SSE framing 错误、`response.failed`、`response.incomplete`、无 terminal EOF 或下游取消，仍必须生成终态记录。首字节已经写出不等于调用成功，也不允许因此重试或 fallback。

### 2.2 终态与错误率

每个已进入路由的请求必须有稳定的 `outcome`，至少区分：

```text
succeeded
rejected_before_egress
rate_limited
temporarily_unavailable
connect_failure
timeout_or_ambiguous
authentication_failure
invalid_request
provider_terminal_error
stream_invalid_or_incomplete
internal_failure
client_cancelled
```

`error_class` 应与[Provider 韧性](provider-resilience.md)的 adapter 分类相对应；无法精确归类时使用稳定的 `internal_failure` 或 `unknown`，不能以原始错误文本形成动态标签。

默认上游/网关终态错误率为：

```text
error_rate = terminal_error_requests / completed_requests
```

其中 `terminal_error_requests` 包含除 `succeeded` 与 `client_cancelled` 外的终态；`completed_requests` 同样排除 `client_cancelled`，并应同时输出分子和分母。`client_cancelled`、入站拒绝和未启用统计期间的请求必须作为独立计数展示，避免把客户端行为或缺失样本伪装成上游错误率。

### 2.3 Usage

若上游在可安全解析的终态中提供 usage，记录并聚合 `input_tokens`、`output_tokens`、`reasoning_tokens`、`cached_tokens` 等字段；缺失时保持 unknown，不以本地估算冒充 Provider 用量。成本估算若启用，必须标记为 owner-maintained estimate，并携带来源和更新时间。

不得记录 prompt/completion、tool arguments/result、完整上游 payload、Authorization、cookie、credential、URL query 或原始错误 body。请求 id 只能存在于逐调用记录或安全日志，不能作为 Prometheus 等聚合指标的 label。

## 3. 输出与资源边界

至少提供一种可被单用户服务所有者消费的稳定 headless 输出：受限的结构化本地记录或 Prometheus-compatible 聚合导出。二者可以并存，但都不构成控制面或 GUI。

- 聚合指标仅使用低基数维度：endpoint、Public Model、Upstream Target、Upstream API、Provider Family、协议、是否流式、route mode、outcome、error class；不使用 request id、原始模型名、错误文本或客户端标识作为 label。
- 逐调用记录使用轮转的本地 JSONL 或等价受限 sink；默认不上传第三方。导出端点若实现，必须只监听 loopback 或复用静态 Bearer/TLS 信任边界。
- 收集和落盘采用有界、非阻塞路径。统计 sink 失败只增加 `telemetry_dropped_records_total` 和安全告警，绝不阻塞模型输出、改变 terminal ownership 或造成无界内存。
- reload 后的统计配置应原子生效；统计开关、sink 路径和保留策略由配置文件定义，不能由业务请求覆盖。

## 4. 最小交付证据

| ID | 应由测试保护的行为 |
|---|---|
| OBS-01 | 成功 JSON、成功 SSE、首输出前失败和已输出后 terminal failure 分别产生正确且唯一的终态。 |
| OBS-02 | SSE 的 `gateway_ttft_ms` 以首个成功写出的 body byte 计时；非流式只产生 `gateway_ttfb_ms`。 |
| OBS-03 | 错误率同时给出分子/分母，`client_cancelled` 与 ingress 拒绝不污染默认上游错误率。 |
| OBS-04 | Provider usage 缺失时保持 unknown；记录和导出中不含正文、secret 或高基数 request id label。 |
| OBS-05 | sink 满、写入失败或导出故障不会延迟、截断或改变下游响应，仅留下有界 dropped 计数。 |
| OBS-06 | 配置 reload 后新请求使用新的 telemetry 设置，进行中的请求保持自己的统计上下文。 |

## 5. 关联文档

- [产品范围](product-scope.md)
- [Provider 韧性](provider-resilience.md)
- [配置与路由](../implementation-plans/configuration-and-routing.md)
- [服务架构](../implementation-plans/service-architecture.md)
- [当前实现说明](../implementation-status/current-implementation.md)
