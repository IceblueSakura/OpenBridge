# Credential Pool、冷却与有限重试综合调研

## 文档元数据

| 字段 | 值 |
|---|---|
| Source snapshot | CLIProxyAPI、LiteLLM 与 cc-switch 三篇前置文档拥有的固定源码 commit 与来源 |
| Last reverified | 2026-08-12：仅对当前前置文档复核综合表述与链接，没有拉取外部仓库或执行 retry 场景 |
| Scope | 比较 credential/account、deployment 与 Provider 三种隔离单位下的 attempt、cooldown 和 failover |
| Evidence boundary | 静态项目行为不能证明真实 Provider quota、跨主机一致性、state migration 或通用 status-code 策略 |
| Recheck trigger | 任一前置项目的 retry/cooldown 实现、资源身份、持久化模型或错误分类发生变化时 |

## 状态与前置文档

本文只比较三个项目已经分别记录的行为：

- [CLIProxyAPI credential retry 与 cooldown](../cliproxyapi/cliproxyapi-credential-pool-retry-analysis.md)
- [LiteLLM deployment retry 与 cooldown](../litellm/litellm-credential-pool-retry-analysis.md)
- [cc-switch request retry 与 failover](../cc-switch/cc-switch-retry-failover-analysis.md)

固定快照和一手来源由各项目文档维护。本文不重复项目源码定位，也不记录任何具体网关的实现状态。

## 1. 共同问题，不同资源单位

三个项目都处理“当前候选失败后是否换另一个候选”，但隔离单位并不相同：

| 项目        | 选择/隔离单位      | 请求内边界                                 | 跨请求状态                        |
|-------------|--------------------|--------------------------------------------|-----------------------------------|
| CLIProxyAPI | credential/account | request retry 与最大 credential 数分别限制 | credential cooldown，可持久化     |
| LiteLLM     | deployment         | retry/fallback 有独立配置                  | deployment cooldown，可结合 Redis |
| cc-switch   | Provider           | `max_retries + 1` 且受 Provider 数量限制   | 持久化 circuit breaker            |

因此，`credential`、`deployment` 和 `Provider` 不能仅因都能“切换”而视为同一故障域。

## 2. 可重复观察

1. **attempt 必须有硬上限。** CLIProxyAPI 把可尝试 credential 数与 request retry 分开；cc-switch 同时受显式最大次数和候选数量限制。
2. **cooldown 是跨请求状态。** 它描述某个资源在一段时间内不应再次被选择，不等于当前请求必须等待其恢复。
3. **健康应隔离到项目定义的最小可替换资源。** LiteLLM 冷却单个 deployment，而不是整个 model group。
4. **错误分类决定是否切换。** cc-switch 不对已归类为客户端输入无效的错误继续 failover；CLIProxyAPI 与 LiteLLM 的 status
   集合则受各自产品策略影响。
5. **候选扩大不能隐式扩大调用预算。** pool/group/provider 列表增长不能自动增加单请求的无限尝试。

## 3. 差异与不可合并项

| 维度          | CLIProxyAPI                        | LiteLLM                   | cc-switch                      |
|---------------|------------------------------------|---------------------------|--------------------------------|
| 429           | credential quota/cooldown 语义较强 | deployment cooldown       | 由 Provider failover 分类处理  |
| 5xx/transport | 可进入 request retry               | deployment retry/fallback | Provider/transport failover    |
| 4xx           | 示例包含部分 4xx                   | 可按错误类型配置          | 客户端无效错误不继续           |
| 分布式协调    | 项目可持久化部分 cooldown          | 可使用 Redis              | 桌面应用持久化 circuit breaker |
| 管理面        | 账号/OAuth 聚合                    | team、budget、virtual key | UI 与客户端配置接管            |

这些差异说明不存在可从三个项目直接抽取的统一 status 表。真正的 retry contract 仍需先确定资源身份、失败作用域、请求是否已产生副作用，以及流式输出是否已经开始。

## 4. 证据边界

- 静态配置和源码观察不证明真实 Provider 的 quota 究竟绑定 API key、账户、组织还是 endpoint。
- account pool、动态权重、预算与 GUI 是各项目产品能力，不是 retry 协议基线。
- stateful continuation 可能额外绑定 account/credential；上述无状态切换观察不能证明 state 可迁移。
- 外部 deterministic tests 只能证明各自实现，不能替代真实 Provider 的 quota 与恢复验证。
