# LiteLLM 调用统计与 Prometheus 调研

## 状态与范围

| 项目           | 值                                                                                        |
|----------------|-------------------------------------------------------------------------------------------|
| 固定源码快照   | `BerriAI/litellm` commit `f955c5e5885e2f6a9ad5ce0304399280794ef1be`，2026-07-25           |
| 当前模块级复核 | commit `23de7a15d9d40006ee596e617475ba101d60c5e9`，2026-08-01                             |
| 阅读范围       | Prometheus integration、logging payload、stream TTFT、failure handler 与 Responses bridge |
| 排除           | 计费正确性、enterprise audit、真实负载开销与多租户部署验证                                |

原始行号只对应固定快照。

## 1. 统计分层

LiteLLM 的 observability 同时包含：

- 请求成功/失败与 token/latency 指标；
- Provider/model/deployment 维度；
- virtual key、team、organization、end-user 与 budget/spend 管理维度；
- callback 和 logging payload 驱动的 Prometheus labels。

这些层次服务于 LiteLLM Proxy 的多租户管理面。模型调用指标、账户归属和计费标签不能视为同一种协议字段。

## 2. TTFT 口径

stream wrapper 在收到第一个可迭代 chunk 时记录 `completion_start_time`，随后形成 TTFT。该时间点受 SDK iterator、Provider
adapter、空 chunk/usage chunk 和 bridge conversion 影响。

因此 LiteLLM TTFT 是具体实现口径；它不自动等于 TCP/HTTP 首字节、首个 SSE frame、首个 text delta 或首个用户可见字符。

## 3. 标签与基数

Prometheus integration 对部分 label series 做限制或清理，但可选维度仍包括 user、team、key、organization、model、Provider 与
deployment。开启哪些 labels 会直接影响基数、隐私和指标可聚合性。

任何采用 LiteLLM 指标的人都需要同时核对：label 来源是否受调用方控制、是否含稳定内部 ID、是否可能包含 credential/user
data，以及 series limiter 的实际配置。

## 4. Failure counter 与终态

Prometheus logger 为 failed request 建立 counter，并从 logging payload 构建上下文。普通 HTTP failure、stream 中的失败
event、EOF、client cancellation、bridge transform error 和一次请求内的多个 Provider attempt 可能落入不同 callback 路径。

所以一个 `failed_requests` counter 的名称不能单独说明其分母、终态唯一性或 streaming failure 分类。

## 5. Responses bridge 的观测边界

Responses 请求可能经过 request transform、Chat upstream、stream transform 和 response
aggregation。中间阶段的异常与最终下游请求状态需要分别观察；否则一次用户请求可能被重复计数，或 transform failure 被成功
upstream call 掩盖。

## 6. 适用边界

- LiteLLM metrics 证明其 callback/payload 如何形成指标，不证明同名指标适用于其他系统。
- team/key/budget/spend 是 Proxy 产品管理维度，不是 OpenAI wire 字段。
- TTFT、latency 与 failure rate 在跨系统比较前必须先对齐事件起止点与分母。
- 静态源码阅读不证明高基数、callback overhead 或真实负载下的指标准确性。

## 一手入口

- [LiteLLM repository](https://github.com/BerriAI/litellm/tree/23de7a15d9d40006ee596e617475ba101d60c5e9)
