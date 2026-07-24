# M02 配置与路由

## 配置对象

- `bootstrap.toml`：监听地址、允许的 origin、请求/SSE 上限和连接池策略；
- `routes.toml`：Provider、credential reference、deployment、capability 和 alias；
- `RegistrySnapshot`：校验后的不可变配置快照；
- `PublicModelAlias`：下游稳定模型名；
- `Deployment`：上游模型、endpoint、credential 和能力；
- `RoutePlan`：单次请求固定的候选、attempt policy 和状态边界；
- `DeploymentAvailability`：进程内、有界、可过期的 cooldown overlay，不修改配置 snapshot。

## 路由规则

1. 解析 public alias 和请求所需 capability；
2. 依配置顺序筛选兼容 deployment；
3. 固定本次请求的 snapshot 和 candidates；
4. 原生协议直接进入 Native Path；
5. attempt 前跳过处于 cooldown 的 deployment；
6. 首输出前的允许失败可在次数、等待和总耗时预算内 retry/fallback；
7. `previous_response_id` 等 provider-bound state 禁止跨 candidate；
8. 恢复耗尽后返回安全的最终 Provider 错误或明确的 all-candidates-cooling-down 429。

## 安全边界

- 下游请求不能提供或覆盖 base URL、credential、认证 header；
- deployment origin 必须在 bootstrap allowlist；
- deployment capability 只能收窄 Provider Family 能力；
- reload 失败不能替换当前快照；
- reload 不改变在途请求。

## 验收

- 同一 snapshot + request 产生确定 candidate set；
- 未知 alias 和不支持能力在 egress 前失败；
- 输出后不 fallback、不拼接另一条 stream；
- 429 cooldown、到期恢复、`Retry-After`、并发更新和 cancel during backoff 通过；
- 最终错误保留 allowlist 内的 status、error fields、request id 和 rate-limit headers；
- state affinity、atomic reload 和配置错误脱敏通过。

## 详细资料

- [本地配置、路由与使用量](../architecture/local-configuration-routing-and-usage.md)
- [目标架构](../architecture/architecture-and-roadmap.md)
- [Provider 韧性需求](../requirements/provider-resilience.md)
- [当前实现](../implementation/current-implementation.md)
