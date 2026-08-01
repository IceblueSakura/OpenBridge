# LiteLLM 调用统计与 Prometheus 边界调研

## 状态与范围

**外部实现调研；用于统计口径和 Prometheus 反例，不导入 LiteLLM Proxy 控制面。**

| 项目 | 值 |
|---|---|
| 调研仓库 | `https://github.com/BerriAI/litellm` |
| 固定证据快照 | `F:/codespace/litellm`，`litellm_internal_staging` @ `b9b27c2beb601433c39dabff3ffcf0248333d49e` |
| 快照日期 | 2026-07-25 |
| 阅读范围 | `litellm/integrations/prometheus.py`、`types/integrations/prometheus.py`、Responses Chat bridge handler |
| 矩阵角色 | usage/error 字段与观测实现的互证参考；OpenBridge 的指标边界仍由自身需求定义 |
| 不在范围 | virtual key、用户/团队/组织/预算、spend、数据库/Redis、UI、callback/control-plane 链路 |

**2026-08-01 当前模块级复核。** 本地 `litellm_internal_staging` 已 fast-forward 至 `23de7a15d9d40006ee596e617475ba101d60c5e9`；`PrometheusLogger`、streaming TTFT、proxy metrics 与 Responses route types 仍可定位。观测实现和行号会随上游演进，故下文逐行证据仍只属于固定快照。

## 1. 已观察到的统计分层

`PrometheusLogger` 在初始化时分别创建 proxy 请求/失败 counter、总请求延迟 histogram、LLM API 延迟 histogram、流式 TTFT histogram 和 token/spend 指标（`integrations/prometheus.py:130-197`）。这说明“请求是否完成”“网关总时延”“上游 API 时延”“首输出时间”“usage”不是同一个指标，也不应依赖单一 duration 推导。

OpenBridge 可采用这种分层思想，但指标名、标签和导出端点不与 LiteLLM 绑定。OpenBridge 只需要稳定的本地 user id，不需要 LiteLLM 的 team/key/organization/spend 管理维度。

## 2. TTFT 的具体口径

LiteLLM 在 `_set_latency_metrics()` 中只对 `stream=True` 记录其 TTFT histogram，起止点为 `api_call_start_time → completion_start_time`（`integrations/prometheus.py:1899-1918`）；非流式请求不会写入该 histogram。它还独立记录 API 调用总时延与总请求时延（`:1920-1968`）。

这提供两个反例：

1. 名为 TTFT 的指标不天然表示“网关向下游写出首字节”；它取决于计时点到底是上游请求、首个 SDK chunk 还是下游 write；
2. 把流式和非流式都塞进同一 TTFT bucket 会丢失语义。

因此 OpenBridge 保持自己的定义：流式记录 `gateway_ttft_ms`（路由开始至首个成功下游 body byte），非流式记录 `gateway_ttfb_ms`；完整终态记录 `gateway_latency_ms`。若日后增加协议感知的“首个文本/reasoning event”，必须单列名称和事件条件。

## 3. 标签稳定性与基数控制

LiteLLM 在 logger 初始化时快照每个 metric 的 label 集合，理由是 Prometheus metric 创建后不能变更 labelnames，运行时开关变化会导致 `.labels()` 不匹配（`integrations/prometheus.py:110-129`）。其实现还存在 `BoundedPrometheusSeriesTracker`，并在默认路径中避免将 end-user 用作 Prometheus 成本追踪维度（`utils.py:9016-9040`）。

OpenBridge 可借鉴的只是安全性质：

- metric label schema 在启动/reload 时验证和固定；
- 无界 request id、原始错误文本、客户端身份、完整模型 URL 或 prompt 不进入 label；
- 若任何可选维度会增大 series，先设上限并单独记录 dropped/overflow。

OpenBridge 不采用 LiteLLM 的 team/API-key、budget、spend 或任意 end-user 标签，即使它们已经过 series 限制；未来统计只能使用用户表中的稳定 user id，不能把调用统计扩展为在线客户端管理。

## 4. 失败计数不等于终态错误率

LiteLLM 的 Prometheus logger 为 failed requests 建 counter，并按 logging payload 构建上下文（`integrations/prometheus.py:1978-2028`）。该机制适合作为“记录失败路径”的工程样本，但不能直接成为 OpenBridge 错误率口径：已开始 SSE 后发生 `response.failed`、`response.incomplete`、EOF 或 client cancellation 可能不等同普通 HTTP failure。

这项调研只保留外部实现观察；OpenBridge 当前尚未实现调用统计，已实现边界见[当前实现说明](../../implementation-status/current-implementation.md)。

## 5. Responses bridge 的观测边界

最新 `LiteLLMCompletionTransformationHandler` 会将 Responses request 转成 Chat request，并把非流式响应或 stream wrapper 再转换回 Responses（`responses/litellm_completion_transformation/handler.py:23-119`）。异步路径在 `previous_response_id` 存在时先进入 session handler（`:80-88`）。

这对 OpenBridge 的含义是：一次“下游请求”可能包含多个内部阶段，但调用统计仍只能有一个下游终态 record；attempt、route mode 和内部 transform 可以作为低基数属性，不能把中间 transform 当成额外的用户调用或把其异常吞进成功样本。

## 6. 需要转化为 OpenBridge 测试的点

| ID | 证据应保护的结果 |
|---|---|
| LOBS-01 | stream 与 non-stream 分别写入 TTFT 与 TTFB，且不混用。 |
| LOBS-02 | 统计标签 schema 在启用前固定；request id/错误文本不产生 Prometheus series。 |
| LOBS-03 | 终态 SSE failure/incomplete 与 HTTP failure 都计入相应 outcome；client cancellation 单列。 |
| LOBS-04 | bridge 的内部 request/response/stream 只形成一个下游 `CallRecord`，其中 route mode 与 attempt 可观测。 |
| LOBS-05 | telemetry 写入失败只增加有界 dropped 指标，不影响下游 stream。 |

## 相关资源

- [项目比较矩阵](../project-comparison.md)
- [LiteLLM Chat/Responses 分析](litellm-chat-responses-analysis.md)
- [LiteLLM Proxy 性能观察](litellm-proxy-performance-bottlenecks.md)
- [当前实现说明](../../implementation-status/current-implementation.md)
- [当前代码架构](../../implementation-status/current-architecture.md)
