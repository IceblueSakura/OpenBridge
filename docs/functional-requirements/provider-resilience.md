# 路由、Provider 韧性与状态亲和需求

## 状态

**Working behavior design。** 当实现涉及多个候选上游、429/5xx、限流或临时失败时，本文提供 TDD 的行为边界；它不属于预定义阶段，也不要求先完成其他方向。

本文定义 Public Model 的 route 选择、上游 Provider 因协议/能力拒绝、RPM/TPM、并发限制、临时过载或服务故障返回错误时的最小恢复行为，以及 continuation 的状态亲和边界。它补充[产品范围](product-scope.md)中的 stream/fallback 边界；实现时从其中一个行为先写失败测试。

OpenBridge 参考 LiteLLM 的“部署可用性过滤、有限 retry/fallback、临时 cooldown”模式，但不复制其多租户 key/team 限流、Redis callback 链、预算或复杂负载均衡控制面。

## 1. 目标

- 对同一 Public Model 以可解释、可重复的规则选择一个满足完整请求语义、上下游协议、转换约束、状态亲和、启用状态和可用性要求的 route；
- 避免持续把新请求发送到已明确限流或临时不可用的 Upstream Target；
- 在不会重复产生业务副作用、尚未向下游输出、且延迟预算允许时执行有限重试；
- 当前 candidate 无法继续时，只在 RoutePlan 允许、能满足同一完整请求且不违反状态亲和的 candidate route 间 fallback；
- 所有尝试耗尽后，向下游返回安全、有效、可诊断的最终错误；
- 保持 state affinity、stream terminal、取消和 secret isolation 不被恢复策略破坏。

## 2. 路由与候选选择

一个 Public Model 绑定代码注册表中的**有序** candidate routes。每条 route 固定 Upstream Target、该 target 下的协议级 Upstream API、下游协议和 Native/Bridge 模式；上游协议由 Upstream API 唯一确定。顺序是默认优先级，不等于这些 candidate 对所有模型、协议、工具或状态都等价。

每个请求应在发起上游调用前形成不可变的 RoutePlan，并按以下顺序筛选：

```text
Public Model routes
→ Upstream Target / Upstream API / upstream protocol / transport
→ Native/Bridge whole-path capability
→ request feature combination / context
→ continuation / tool state affinity
→ enabled 与当前 cooldown
→ 配置顺序选择
```

- capability 必须按 Model、Upstream Target、协议级 Upstream API、上下游协议、converter、route-local ConversionPolicy 与完整 feature combination 判断；`Unknown` fail closed，不能因 Provider 名称、另一条 route 的独立字段或一次成功猜测支持。
- route selection 只使用受信配置、当前 availability overlay 和请求中为兼容判断必需的语义；不能使用 prompt、用户标识、secret、随机权重、未审查 cost 或隐式账号轮换。
- `previous_response_id`、Provider resource、tool continuation、Codex turn state、opaque reasoning 或无法重建的历史会把请求绑定到 issuing target/upstream API。没有可验证 ledger 时，候选切换不是有效降级。
- RoutePlan 一旦形成，在整个 request/stream 内保持 Public Model、route、Upstream Target/Upstream API、credential binding、协议模式、BridgePlan、candidate 顺序和 fallback 边界；配置更新只影响后续请求。
- 模型目录的 `context_length.input`、`context_length.output` 或其他模型限制只能在有可靠模型事实和可解析请求字段时用于保守筛选；不能以 JSON 字节数或猜测用量伪造 token 预检。

## 3. 非目标

- 面向下游用户、团队或 API key 的 RPM/TPM 配额系统；
- 精确复制 Provider 的全局 token bucket，或承诺本地计数等于 Provider 账单计数；
- Redis/数据库驱动的分布式限流、健康检查集群或多实例强一致 cooldown；
- 动态成本优化、权重学习、无限 retry、请求排队系统；
- 同 Provider 多 credential pool、账号轮换或通过换账号绕过限流；
- 对已经产生业务输出的多个响应进行拼接。

可选的 owner-configured RPM/TPM/concurrency hint 只能用于保守的本地 admission/pacing；它不是 Provider 配额真相，不能替代 429、`Retry-After` 和 Provider rate-limit header。

## 4. 术语与状态

| 名称 | 含义 |
|---|---|
| Attempt | 对一个确定 Execution Plan/Upstream Target/Upstream API 的一次上游 HTTP 调用。 |
| Same-candidate retry | 在同一 Upstream Target、Upstream API、credential binding、协议和转换路径上重新调用。 |
| Fallback | 按不可变 RoutePlan 转到下一个已批准且仍满足完整请求的 candidate route。 |
| Cooldown | 在一个有界时间内阻止新的无状态请求选择某 Upstream Target 或明确共享 quota scope。 |
| Retry budget | 对 attempt 次数、等待时间和总耗时的共同上限。 |
| Observable output | 已交给下游的业务 JSON、SSE event 或 response body bytes。 |

最小 Upstream Target 运行时状态：

```text
Available
  → CoolingDown(until, reason, source_attempt)
  → Available
```

cooldown 是运行时 availability overlay，不修改配置快照或 capability。RoutePlan 保持 candidate identity、顺序、credential binding 和 fallback 边界不变；attempt manager 在每次调用前读取当前 availability。

## 5. 错误分类

Provider adapter 必须先分类，再决定 retry、cooldown 或直接返回：

| 错误类别 | 典型证据 | Same-candidate retry | Cooldown | Fallback |
|---|---|---|---|---|
| `rate_limited` | HTTP 429、明确 Provider rate-limit error | 仅在未输出、拒绝已确定且预算允许时 | 是 | 无 state affinity 时可 |
| `temporarily_unavailable` | adapter 明确认可的 502/503/504、overloaded event | 仅在未输出且重复安全时 | 是，可短于 429 | 无 state affinity 时可 |
| `connect_failure` | connect/DNS/TLS 建连失败 | 仅在未输出且预算允许时 | 可选短冷却 | 无 state affinity 时可 |
| `timeout_or_ambiguous` | 上游可能已接收请求后的 timeout/断连 | 默认不重试；需契约证明幂等 | 默认否或极短 | 默认否 |
| `authentication` | 401/403、invalid credential | 否 | 否；应标记配置/凭证故障 | 否，除非候选使用独立且允许的 credential |
| `unsupported_protocol_or_capability` | adapter 从安全 status/body/error code 明确认定当前上游路径不支持请求语义 | 否 | 否 | 首输出前且存在满足同一完整请求、不违反状态亲和的 route 时可 |
| `invalid_request` | 与能力无关的 400/404/409/422 或 schema/request 错误 | 否 | 否 | 否 |
| `provider_terminal_error` | 已开始的 SSE 中出现 failed/error/incomplete | 否 | adapter 可记录健康信号，但不得影响当前 stream | 否 |

状态码只是默认线索。Provider Family adapter 可以依据安全响应 body、error code 和官方 header 收窄分类，但不能通过 Provider 名称字符串在核心 router 中增加启发式分支。

## 6. Retry budget

每个请求必须同时受以下边界约束：

- `max_attempts_per_candidate`：`【需根据实际情况完善】`；
- `max_total_attempts`：`【需根据实际情况完善】`；
- `max_retry_delay`：单次等待上限，`【需根据实际情况完善】`；
- `max_retry_elapsed`：从首次 attempt 到停止恢复的总时间上限，`【需根据实际情况完善】`；
- 下游 disconnect/cancel 立即终止等待和后续 attempt；
- 请求 deadline/Provider timeout 的剩余时间不足时不再 retry。

必须使用小且明确的默认值。配置可以收紧这些值；放宽必须受全局安全上限约束，不能配置为无限。

对于有效的 `Retry-After` 或 Provider rate-limit reset：

1. 解析为绝对恢复时间；
2. 应用本地最大 cooldown 上限；
3. 若等待时间不超过当前请求的 retry delay/elapsed budget，可在同 candidate 等待后重试；
4. 否则立即将 Upstream Target 置为 cooldown，并评估下一个 candidate；
5. 没有可用 candidate 时向下游返回带有效 `Retry-After` 的最终错误，不长时间占住连接。

没有有效 header 时使用有界 exponential backoff + jitter；默认 base、cap 和 jitter 为 `【需根据实际情况完善】`。

## 7. Cooldown 规则

### 7.1 最小作用域

- 第一版以稳定 `upstream_target_id` 为最小 cooldown key；
- 同一 Upstream Target 的 cooldown 对后续无状态请求可见；
- 不自动推断多个 Upstream Target 共享同一 Provider account/RPM/TPM bucket；
- 只有受信配置或 Provider 契约明确声明共享 quota scope 时，才能扩大到 credential/account/model scope。

### 7.2 触发与恢复

- 429 必须触发 cooldown；
- 明确 retryable 的 overloaded/503 可触发较短 cooldown；
- 400/401/403、capability 拒绝和本地配置错误不得触发普通 rate-limit cooldown；
- 新的更晚恢复时间可以延长 cooldown，但不能无限累加；
- 到期后自动重新参与 selection；第一版不要求后台 health probe；
- 成功 attempt 可清除同 scope 的临时失败计数；
- 配置更新后仅在 target identity、credential binding 和 quota scope 未变化时保留 cooldown，否则丢弃旧状态。

进程重启后允许丢失第一版 cooldown 状态；该限制必须记录，不得宣称多实例或重启后一致。

### 7.3 State affinity

- `previous_response_id`、Provider resource、tool continuation 或 issuing call 绑定的请求不能因 cooldown 转到其他 target/upstream API；
- 若 issuing Upstream Target 正在 cooldown，只有“同 target/upstream API retry”或直接返回错误两种结果；
- cooldown 不能把有状态请求误判为无 candidate 后静默降级；
- 已输出任何业务内容后，当前请求不再 retry/fallback，即使同时触发了 cooldown。

## 8. 最终错误传播

### 8.1 尚未输出业务响应

最终错误应尽量保留最后一个有意义上游失败的：

- HTTP status；
- OpenAI-compatible error body 中安全的 `message`、`type`、`code`、`param`；
- `Content-Type`；
- `Retry-After`；
- Provider 允许公开的 request id；
- `x-ratelimit-*` 或 adapter allowlist 中等价的非敏感 rate-limit header。

不得转发 credential、cookie、内部 URL、认证 header、完整 debug stack 或未经审查的敏感响应 header。

若所有 candidate 都因 cooldown 被跳过、当前请求没有真实上游响应，OpenBridge 返回 OpenAI-compatible 429：

```json
{
  "error": {
    "message": "All eligible upstream targets are temporarily rate limited or unavailable.",
    "type": "upstream_temporarily_unavailable",
    "code": "openbridge_all_candidates_cooling_down"
  }
}
```

同时返回 proxy request id；若能确定最早恢复时间，返回有上限的 `Retry-After`。具体用户可见文本允许本地化，但 `type`/`code` 必须稳定。

### 8.2 已输出业务响应

- Native Path 保留目标协议可表达的 Provider error/terminal；
- Bridge Path 只能映射为目标协议已有的失败语义；
- 不注入目标客户端不认识的自定义 SSE event；
- 不 retry、不 fallback、不拼接另一 candidate；
- 若 wire protocol 无法在已开始 stream 后表达详细错误，则关闭 stream，并在安全日志中记录 request id、target/upstream API、错误分类和 terminal outcome。

### 8.3 多次失败的选择

- 对外返回最后一个实际 attempt 的、最能代表最终失败的安全错误；
- 若最后一个错误是 OpenBridge 本地 timeout/cancel，不得用更早的 429 覆盖；
- 结构化日志记录每个 attempt 的 target/upstream API、序号、分类、等待、cooldown 决策和 request id，但不记录请求/响应正文或 secret；
- 下游错误不得暴露完整候选列表或内部 credential identity。

## 9. 并发与资源要求

- cooldown 状态更新必须原子化，避免并发 429 后仍发生无界请求风暴；
- 同一请求的 retry sleep 可取消，且不占用不必要的并发 permit；
- 第一版允许进程内有界 registry，不要求 Redis；
- registry 必须限制 entry 数和过期状态保留时间；
- 不能逐 token 更新 RPM/TPM 存储；usage 只在请求完成后作为观测数据；
- 可选本地 RPM/TPM pacing 的 token 估算误差、共享账号外部流量和多实例偏差必须显式记录。

## 10. 建议的行为测试矩阵

| ID | 应由测试保护的行为 |
|---|---|
| RES-01 | 429 + `Retry-After` 产生 Upstream Target cooldown，新的无状态请求在到期前跳过它。 |
| RES-02 | 无 header 的 429 使用有界 backoff/jitter，attempt 次数与总耗时不会超过配置。 |
| RES-03 | cooldown 中存在下一个等价 candidate 时，按 RoutePlan 顺序 fallback。 |
| RES-04 | 全部 candidate cooling down 时返回稳定 429 code 和有效 `Retry-After`，不调用上游。 |
| RES-05 | 400/401/403/invalid request 不进入普通 retry/cooldown。 |
| RES-06 | `previous_response_id`、tool continuation 和 Provider resource 不跨 Upstream Target/Upstream API。 |
| RES-07 | 已输出 JSON/SSE 后的 error/EOF 不 retry、不 fallback、不拼接。 |
| RES-08 | 最终 429/5xx 保留 allowlist 内的 status、error fields、request id 和 rate-limit headers。 |
| RES-09 | cancel 会中止 backoff wait 和剩余 attempt。 |
| RES-10 | 并发 429、cooldown 到期、注册表重建后的 identity 变化和 registry 上限通过确定性测试。 |
| RES-11 | 同一 Public Model 的 candidate routes 按完整 feature combination、协议/转换、state affinity、enabled/cooldown 与配置顺序确定性选择；`Unknown` capability 不会出站。 |
| RES-12 | 上游在首输出前明确拒绝某条 route 的协议/能力时，只 fallback 到仍满足同一完整请求的已批准 route；所有安全候选耗尽后才返回归一化的最终不支持错误。 |
| RES-13 | 模型限制只在可靠、已配置的字段与明确请求上限下保守筛选，不伪造 context token 计数。 |

## 11. 与 LiteLLM 的边界

采用：

- 在选择 Upstream Target 前过滤临时不可用项；
- 对 retry/fallback 设置明确次数和时间预算；
- 根据 rate-limit/temporary failure 更新部署可用性；
- 保留 Provider-specific error/retry 分类。

不采用：

- key/team/user/project 多层 RPM/TPM；
- 每请求 Redis callback 限流链；
- 多 credential rotation；
- 复杂 routing strategy、预算和计费控制面；
- 将所有 429/5xx 一律 retry 的宽泛策略。

相关源码调研见 [LiteLLM Proxy 请求链分析](../references/litellm/litellm-proxy-call-chain-analysis.md)和[性能瓶颈分析](../references/litellm/litellm-proxy-performance-bottlenecks.md)。

## 12. 关联文档

- [产品范围](product-scope.md)
- [网关 API 与客户端兼容](gateway-api-compatibility.md)
- [配置、凭证与受信运行边界](configuration-and-credentials.md)
- [调用统计与可观测性](observability.md)
- [配置与路由](../implementation-plans/configuration-and-routing.md)
- [Provider 适配与数据流](../implementation-plans/provider-adapters-and-dataflow.md)
- [服务架构](../implementation-plans/service-architecture.md)
- [当前实现说明](../implementation-status/current-implementation.md)
