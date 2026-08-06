# 当前开发焦点

## 状态

**活动焦点：默认禁用的 loopback OTLP/HTTP trace 导出闭环。** 本焦点只建立 OpenTelemetry 共用的配置、exporter 生命周期和
request/attempt trace 边界，不并行实现 metrics、logs 或移除现有进程内统计。

### 行为

当配置所有者在 bootstrap 中显式启用合法的 loopback OTLP/HTTP trace exporter 时，一个只启动一次 Provider attempt 的已认证
非流式 Chat Completions 请求会向 collector 导出一个脱敏 `downstream_request` root span 和一个
`provider_attempt` child span；未配置时不产生 OTLP egress，collector 不可达时下游 HTTP/SSE 结果仍与未启用 exporter 时一致。

### 对应功能需求

[网关 API 与客户端兼容需求中的运行期观测与 OpenTelemetry 导出](../functional-requirements/gateway-api-compatibility.md#7-运行期观测与-opentelemetry-导出)，
重点验收 `OBS-01`、`OBS-02`、`OBS-05`、`OBS-06`；bootstrap 所有权同时受
[配置与凭证](../functional-requirements/configuration-and-credentials.md)的 `CFG-06` 约束。

### 先失败的测试或复现

- `bootstrap_accepts_explicit_loopback_otlp_trace_export_and_rejects_non_loopback`：向 bootstrap 加入显式 OTLP trace 配置；当前
  `deny_unknown_fields` schema 没有 telemetry 字段，合法配置也会解析失败。
- `otlp_http_exports_one_redacted_request_and_attempt_trace`：使用无密钥 Router fixture 发起一个只执行一次 Provider attempt 的非流式
  Chat Completions 请求，由 loopback fake collector 解码 OTLP/HTTP payload 并断言父子关系、终态和敏感字段缺失；当前没有
  OpenTelemetry SDK/exporter，
  `upstream_attempt` 也只是 request span 内的 event，因此不会收到目标 trace。
- `disabled_or_unavailable_collector_does_not_change_gateway_response`：分别在 exporter 未启用和 collector 阻塞/失败时执行相同请求，
  断言业务响应、attempt 次数和取消边界一致且在有界时间内结束；当前不存在 exporter runtime，无法证明这项隔离契约。

### 最小实现边界

- 在 bootstrap schema v2 增加可选、默认禁用且只接受 loopback OTLP/HTTP 的 trace 配置；字段缺失等价于完全禁用，不更新
  `schema_version`。本焦点不提供业务请求覆盖、自定义 header、远程 collector 或动态 reload。队列、batch、export timeout 与
  shutdown flush 使用代码内有界值，避免扩展成通用 exporter DSL。
- 引入最小 OpenTelemetry trace/OTLP HTTP 依赖并一致更新 `Cargo.lock`；保留现有 `tracing-subscriber` fmt 输出，通过组合 layer
  同时交付本地日志和 OTLP trace。
- 项目尚未发布；为符合 OpenTelemetry SDK、`tracing` 和 Tokio/Axum 生命周期最佳实践，可以直接重组 startup、observability、
  `GatewayState` 或内部 public crate API，不保留旧初始化路径、兼容 facade 或双实现。该权限只服务于本 trace 行为，不扩大到 metrics/logs。
- 让进程持有 tracer provider/export worker 直到 Axum 停止，并在关闭时执行有界 flush；初始化失败按配置错误在 listener 前结束，
  运行期 export 失败只产生限频本地诊断。
- 复用现有 `downstream_request` 生命周期作为 root span；为一个实际 Provider attempt 建立一个 child span，并在 HTTP failure、
  transport failure、正常 body terminal、stream failure 或 cancel 的现有唯一终态边界结束它。
- OTLP attribute 使用显式 allowlist：稳定 operation、Public Model、编译期 Provider/Target/Route、route mode、streaming、低基数
  outcome 和直接观测 timing/usage；保留 request id 用于 trace 关联，但不导出 user id、raw path/query、credential、body、tool/reasoning
  内容、真实 endpoint 或原始错误正文。

明确不做：OTLP metrics、OTLP logs、修改 `GatewayMetrics`/`ProviderMetrics`、改变或移除
`/openbridge/v1/metrics*`、历史存储/SQLite/dashboard、Prometheus、W3C trace-context 接收或向上游传播、remote collector、exporter
认证、路由/重试/fallback 策略变化，以及以本焦点声称真实 Provider、负载或长稳性能已经验证。

### 本次验证

- 本地测试：先运行新增 bootstrap/OTLP focused tests，再运行 `cargo fmt -- --check`、`cargo test --locked`、
  `cargo clippy --locked -- -D warnings` 与 `git diff --check`。
- SDK、独立客户端或 source-compatible profile：不绑定 OpenAI SDK；使用独立 loopback fake OTLP/HTTP collector 验证 payload、
  parent/child、禁用态零 egress、脱敏和 collector failure 隔离。若本机已有官方 OpenTelemetry Collector，再记录其实际版本并补一次
  无密钥 smoke；未安装时明确记为未运行。
- 如需真实 Provider/Hermes，说明原因：本焦点不需要。协议 fixture 足以触发 request/attempt 生命周期；真实 Provider、Hermes、
  load、benchmark 与 long-run 均不作为本焦点完成证据。

每次运行记录实际 SDK/客户端版本、安装或源码来源、平台、无密钥配置和脱敏结果；不使用长期固定版本。

### 结果记录

- 已证明的事实（完成后写入[当前实现说明](../implementation-status/current-implementation.md)）：实际配置 shape、默认禁用行为、
  OTLP/HTTP trace shape、attribute allowlist、failure/backpressure 隔离、关闭边界，以及各条验证命令和依赖/collector 版本。
- 仍未知或需另起焦点的事项：Responses/Embeddings/streaming trace 覆盖、OTLP metrics instruments、OTLP logs 过滤与关联、多
  attempt/retry/fallback trace 验收、W3C context propagation、现有自有累计/HTTP endpoint 的缩减、外部持久化与分析程序、显式
  下游 tool outcome 契约，以及真实负载下的开销和长稳表现。

## 关联文档

- [产品范围](../functional-requirements/product-scope.md)
- [网关 API 与客户端兼容](../functional-requirements/gateway-api-compatibility.md)
- [配置、凭证与受信运行边界](../functional-requirements/configuration-and-credentials.md)
- [路由与 Provider 韧性](../functional-requirements/provider-resilience.md)
- [交付与证据要求](../functional-requirements/delivery-and-evidence.md)
- [当前实现说明](../implementation-status/current-implementation.md)
