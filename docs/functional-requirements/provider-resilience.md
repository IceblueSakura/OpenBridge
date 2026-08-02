# 路由与 Provider 韧性

## 状态

本文只描述当前代码已具备或必须保持的 Route 选择、有限 retry/fallback、单进程短时 cooldown 与状态亲和
边界。动态权重、持久化健康、跨进程协调和分布式限流尚未实现，不在本文预先设计。

## 当前路由边界

- 下游只选择 Public Model，不得指定 Provider、Upstream Target、Upstream API、endpoint 或 credential；
- Public Model 按配置顺序提供完整 Route，每条候选必须独立满足协议、能力、模型限制和 reasoning 要求；
- 不同 Route 的能力不能按字段求并集；未知能力不得出站尝试；
- RoutePlan 在请求开始后保持固定，不因一次上游响应重新解析 Public Model；
- `previous_response_id` 等 Provider-bound state 禁止跨 Upstream Target fallback；非空 ID 只有在 issuing
  Upstream Target/Upstream API 可由配置唯一确定时才能形成候选，否则在 egress 前拒绝；
- `store: true` 只允许进入明确支持该能力的 Native Responses Route，不得进入 Bridge 或通过字段删除降级为
  无状态调用。

## 当前 retry 与 fallback

当前在 stream/non-stream 请求尚未向下游提交 response 前执行有限 attempt：

- 429、明确的 5xx、连接失败或 timeout 可按 adapter 分类进入有限 retry；
- 所有候选共享请求级硬预算；每个候选有独立局部上限，且局部 retry 不能无界挤占尚未尝试的候选；
- retry 与 fallback 之间使用 capped exponential backoff，等待随下游任务取消；
- 只有 RoutePlan 允许 fallback 时才能进入下一条完整候选；“同模型其他 Provider”指同一 Public Model 已配置且通过完整 capability/state gate 的 Route，不按模型字符串猜测等价性；
- 有状态 Responses 不进入跨 Target fallback；不能把另一个支持同模型或同协议的 Target 当作原 response ID 的
  issuing target；
- 认证失败、无效请求和本地能力拒绝不应作为普通 transient failure 重试；
- 一旦向下游提交 response，不得 retry、fallback 或拼接另一上游响应；
- 下游取消应终止 pending send、当前上游 stream、退避 timer 和尚未开始的后续尝试。

## 当前跨请求健康隔离

- `429` 将当前 target 的短时 cooldown 记录到 `quota_scope`；暂时性 5xx、timeout 与 transport failure 记录到
  `fault_domain`；未显式配置 scope 时只隔离 target 自身；
- `Retry-After` 支持 delta-seconds 与 HTTP-date，缺失时使用 1 秒默认值，任何单次建议都截断到 30 秒；
- 后续无状态请求跳过任一 scope 仍在 cooldown 的 candidate；同一请求的局部 retry 仍由 `AttemptManager` 控制，
  不因跨请求状态突破 attempt 上限；
- 成功 HTTP response 清除该 target 所属 scope 的 cooldown；状态只在当前进程内存中保存，不形成持久化健康结论；
- target-bound continuation 即使 target 正在 cooldown 也继续尝试原 target，并保持禁止跨 target fallback；
- 认证失败、普通无效请求和其他不可重试 4xx 不进入 cooldown，也不触发 credential 轮换。

当前实现不提供动态权重、后台探测、持久化健康状态、跨进程协调、分布式限流或多 credential pool。有限
retry/fallback 与短时 cooldown 不能被描述成完整的 Provider 韧性系统。

## 错误传播

- 下游 response 提交前的最终失败保留最后一个可安全传递的 HTTP status、OpenAI-compatible error 字段、`Retry-After` 和 allowlist header；最终 transport timeout/error 使用稳定的网关错误；
- 不得转发 credential、cookie、内部 URL、认证 header、完整堆栈或未经审查的响应 header；
- 已开始的 SSE 只使用目标协议已有的错误、terminal 或连接关闭语义，不注入私有事件；
- 多次 attempt 后返回最后一个最能代表最终失败的安全错误，不向下游暴露候选列表。

## 当前验证重点

- Route 按完整能力组合确定性选择；
- `store: true` 与非空 `previous_response_id` 只进入能力已声明且 issuing target 可唯一确定的 Native Route；
- 有状态 Responses 不进入 Bridge 或跨 target fallback；
- stream/non-stream 提交下游 response 前的 retry/fallback 具有 request-wide 硬上限和指数退避；
- 下游取消 pending send 或退避时不会启动后续 attempt；
- 首输出后错误、EOF 与取消不会触发拼接；
- 429/5xx 的安全 status、error body 和 `Retry-After` 处理保持稳定。
- 共享 `quota_scope`/`fault_domain` 会隔离后续无状态请求，target-bound continuation 不因 cooldown 漂移。

已覆盖的测试源码与最近实际运行的验证范围见[当前实现说明](../implementation-status/current-implementation.md)。

## 关联文档

- [网关 API 与客户端兼容](gateway-api-compatibility.md)
- [配置、凭证与受信边界](configuration-and-credentials.md)
- [当前代码架构](../implementation-status/current-architecture.md)
- [当前实现说明](../implementation-status/current-implementation.md)
