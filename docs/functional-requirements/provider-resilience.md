# 路由与 Provider 韧性

## 状态

本文只描述当前代码已具备或必须保持的 Route 选择、有限 retry/fallback 与状态亲和边界。跨请求 cooldown、
动态权重、分布式限流和独立 `AttemptManager` 尚未实现，不在本文预先设计。

## 当前路由边界

- 下游只选择 Public Model，不得指定 Provider、Upstream Target、Upstream API、endpoint 或 credential；
- Public Model 按配置顺序提供完整 Route，每条候选必须独立满足协议、能力、模型限制和 reasoning 要求；
- 不同 Route 的能力不能按字段求并集；未知能力不得出站尝试；
- RoutePlan 在请求开始后保持固定，不因一次上游响应重新解析 Public Model；
- `previous_response_id` 等 Provider-bound state 禁止跨 Upstream Target fallback。

## 当前 retry 与 fallback

当前仅在流式请求尚未写出第一个下游业务 body 前执行有限 attempt：

- 429、明确的 5xx、连接失败或 timeout 可按 adapter 分类进入有限 retry；
- 只有 RoutePlan 允许 fallback 时才能进入下一条完整候选；
- 认证失败、无效请求和本地能力拒绝不应作为普通 transient failure 重试；
- 一旦写出业务 body，不得 retry、fallback 或拼接另一上游响应；
- 下游取消应终止当前上游 stream 和尚未开始的后续尝试。

当前实现不提供跨请求 cooldown、共享 quota scope 调度、持久化健康状态、动态权重或多 credential pool。一次
请求内的固定次数 retry 不能被描述成完整的 Provider 韧性系统。

## 错误传播

- 首输出前最终失败保留安全的 HTTP status、OpenAI-compatible error 字段、`Retry-After` 和 allowlist header；
- 不得转发 credential、cookie、内部 URL、认证 header、完整堆栈或未经审查的响应 header；
- 已开始的 SSE 只使用目标协议已有的错误、terminal 或连接关闭语义，不注入私有事件；
- 多次 attempt 后返回最后一个最能代表最终失败的安全错误，不向下游暴露候选列表。

## 当前验证重点

- Route 按完整能力组合确定性选择；
- `previous_response_id` 不跨 target fallback；
- 首输出前 retry/fallback 有界；
- 首输出后错误、EOF 与取消不会触发拼接；
- 429/5xx 的安全 status、error body 和 `Retry-After` 处理保持稳定。

已覆盖的测试源码与最近实际运行的验证范围见[当前实现说明](../implementation-status/current-implementation.md)。

## 关联文档

- [网关 API 与客户端兼容](gateway-api-compatibility.md)
- [配置、凭证与受信边界](configuration-and-credentials.md)
- [当前代码架构](../implementation-status/current-architecture.md)
- [当前实现说明](../implementation-status/current-implementation.md)
