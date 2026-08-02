# Credential Pool、冷却与有限重试对照

## 状态与范围

**外部实现调研；只为 OpenBridge 的 API-key pool、HTTP 429 冷却和请求级 attempt 边界提供反例与对照，
不是实现模板。** 本文不采用参考项目的 OAuth/订阅账号聚合、GUI、数据库、预算、动态 Provider 配置或
跨进程控制面。

| 项目 | 固定快照 | 阅读范围 | 本文角色 |
|---|---|---|---|
| CLIProxyAPI | `main` @ `bc71c77f5cc42f3fbe1bf040cf14d4f166894835`，2026-08-02 | `config.example.yaml`、credential cooldown 文档 | credential 尝试上限、round-robin 与 cooldown 的直接对照 |
| LiteLLM | `main` @ `de706a35a6f1e9cb8c3cb527271df0b76a69f410`，2026-08-02 | Router retries、deployment cooldown、fallback 文档 | 将失败隔离到单个 deployment，而不是整个 model group |
| cc-switch | `main` @ `ebbf141fc71547a99f669df1be8e345130d1d890`，2026-08-02 | `src-tauri/src/proxy/forwarder.rs`、failover 文档 | 请求级最大尝试数、错误分类和 circuit breaker 反例 |

## 观察事实

### CLIProxyAPI

- `config.example.yaml` 将 `request-retry`、`max-retry-credentials`、`max-retry-interval` 与 cooldown 开关分开；
  说明单请求 attempt、不同 credential 数量和跨请求健康不是同一个预算。
- 认证文档描述 quota failure 后冷却当前 credential、改用下一个 credential，并在 cooldown 到期后重新加入
  rotation；默认 routing 策略是 round-robin。
- 当前示例把 `403/408/500/502/503/504` 也放入 request retry，并支持持久化 cooldown。该范围依赖它的
  多账号/OAuth 产品，不适合直接成为 OpenBridge 的 API-key 规则。

### LiteLLM

- Router 对多个 deployment 做负载选择；cooldown 作用于单个 deployment，其他同 model group deployment
  仍可使用。
- 429 会立即触发 deployment cooldown；retry 与 cooldown 分开配置，并允许按错误类型设置重试次数。
- LiteLLM 还包含 Redis、预算、团队/virtual key 和动态配置。OpenBridge 只采用“健康隔离到最小可替换
  资源”和“所有路径仍有硬 attempt 上限”两条原则。

### cc-switch

- `RequestForwarder` 将 `max_retries` 转换为 `max_attempts = max_retries + 1`，循环同时受 provider 数量限制；
  这避免配置规模无限放大单请求调用次数。
- forwarder 区分 Provider/transport failure 与经整流后仍无效的客户端请求；后者不会继续 failover。
- cc-switch 的 failover 单位是 Provider，并带 UI、持久化 circuit breaker 和客户端配置接管。它不能证明
  OpenBridge 应把 credential 当作 Provider 或 Route，也不能证明所有 4xx 都应轮转。

## 适用于 OpenBridge 的最小规则

1. 一个共享 `CredentialPool` 是 Provider-scoped、可被多个 Upstream Target 引用的受信资源；单个 key 的
   cooldown 不得直接污染整个 target、quota scope 或 Public Model。
2. 仅 HTTP 429 在首输出前触发 API-key 轮转。429 冷却当前 credential；5xx、timeout 与 transport failure
   继续作用于 target/fault domain；其他 4xx 不轮转。
3. key 轮转、同 candidate retry 与 Route fallback 共享现有请求级硬预算和同一 capped exponential backoff；
   pool 大小不得扩大 attempt 上限。
4. 每个请求不得因 429 回到已在该请求中拒绝过的 credential；跨请求则按 cooldown deadline 自动恢复。
5. `Retry-After` 只决定失败 credential 的跨请求 cooldown，不要求当前请求等待该 key 恢复；存在其他可用
   key 时仅等待正常 attempt backoff。
6. 所有 credential 都不可用时优先进入下一条完整 Route；没有 Route 时保留最后一个安全 429，纯粹因既有
   cooldown 无法开始任何 attempt 时返回稳定的 cooldown 错误。
7. pool/member ID 可以进入脱敏 trace，但 secret、locator 与 Authorization 不得进入错误、日志、metrics、
   fixture 或 probe report；低基数 metrics 只累计轮转次数与终态。

## 不适用与待真实验证边界

- HTTP 429 不能证明配额是 key 级、账号级还是 Provider 级；第一版只采用可预测的 key-local 假设，并在
  pool 全部不可用时收敛，不解析 Provider 错误正文。
- 不引入余额查询、HTTP 402 特判、401/403 credential spraying、动态权重、failure-rate 阈值、后台 probe、
  cooldown 持久化或跨进程协调。
- `previous_response_id` 等 target-bound state 可能同时绑定 credential/account。没有 credential affinity
  证据或 ledger 时，多成员 pool 不得用于这类 Upstream API。
- 外部项目的 deterministic tests 不证明 DeepSeek、MiMo 或其他真实 Provider 的 quota 作用域；真实 Key
  验证必须单独执行并记录，不进入默认测试基线。

## 一手入口

- [CLIProxyAPI `config.example.yaml`](https://github.com/router-for-me/CLIProxyAPI/blob/bc71c77f5cc42f3fbe1bf040cf14d4f166894835/config.example.yaml)
- [CLIProxyAPI authentication 与 cooldown](https://router-for-me-cliproxyapi.mintlify.app/concepts/authentication)
- [LiteLLM Router](https://docs.litellm.ai/docs/routing)
- [LiteLLM reliability](https://docs.litellm.ai/docs/proxy/reliability)
- [cc-switch `forwarder.rs`](https://github.com/farion1231/cc-switch/blob/ebbf141fc71547a99f669df1be8e345130d1d890/src-tauri/src/proxy/forwarder.rs)
