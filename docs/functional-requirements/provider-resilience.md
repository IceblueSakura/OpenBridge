# 路由与 Provider 韧性

## 状态

本文描述当前代码已具备或必须保持的 Route 选择、有限 retry/fallback、单进程短时 cooldown 与状态亲和
边界，并定义已经实现的同 Provider API-key pool 行为。实现事实仍以实施现状为准；动态权重、持久化
健康、跨进程协调和分布式限流尚未实现。

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

- `429` 只将当前 credential member 记录到 pool cooldown；暂时性 5xx、timeout 与 transport failure 记录到
  `fault_domain`，未显式配置时只隔离 target 自身；
- `Retry-After` 支持 delta-seconds 与 HTTP-date，缺失时使用 1 秒默认值，任何单次建议都截断到 30 秒；
- 后续无状态请求跳过仍在 cooldown 的 member 或 fault domain；同一请求的局部 retry 仍由 `AttemptManager` 控制，
  不因跨请求状态突破 attempt 上限；
- 成功 HTTP response 只清除当前 member 和 target 自身的 cooldown，不清除其他 member；状态只在当前进程内存中保存，不形成持久化健康结论；
- target-bound continuation 要求单 member pool，即使 target 正在 cooldown 也继续尝试原 target，并保持禁止跨 target fallback；
- 认证失败、普通无效请求和其他不可重试 4xx 不进入 cooldown，也不触发 credential 轮换。

本需求不把动态权重、后台探测、持久化健康状态、跨进程协调、分布式限流或动态 credential 控制面纳入当前
承诺。有限 retry/fallback 与短时 cooldown 不能被描述成完整的 Provider 韧性系统。

## API-key pool 的错误、轮转与退避策略

本节描述当前必须保持的行为要求。外部项目对照与不采用范围见
[Credential Pool、冷却与有限重试对照](../references/cross-project/credential-pool-retry-analysis.md)。

### 失败分类与作用域

| 上游结果 | 是否重试 | 下一 attempt | 跨请求健康作用域 |
|---|---|---|---|
| HTTP `429`，且尚未提交下游 response | 是 | 同 Route/Target 的下一个可用 credential | 当前 pool member |
| HTTP `5xx` | 是 | 按现有 `AttemptManager` 重试当前 candidate；不因 status 更换 credential | target 的 `fault_domain` |
| transport request failure 或 timeout | 是 | 按现有 candidate retry/fallback；不更换 credential | target 的 `fault_domain` |
| HTTP `400/401/402/403/404/408/409/422` 或其他非 429 `4xx` | 否 | 立即返回当前安全错误 | 不记录 cooldown |
| 已提交下游 response、SSE 已输出 event 或 body 已开始 | 否 | 禁止 retry、credential rotation 或 Route fallback | 只记录终态 |

第一阶段只按 HTTP status 分类，不读取 Provider error body，不把 `402` 推断为余额不足，也不根据错误字符串、
私有 header 或模型名猜测 quota。`401/403` 不轮询其他 key，避免 credential spraying 和错误配置被掩盖。

### Credential 选择

- 每个 pool 维护跨请求共享、并发安全的 round-robin cursor；新 attempt 从 cursor 开始选择第一个未冷却且
  未被本请求 429 拒绝的 member，选择后推进 cursor；
- 同一请求可以在 429 后选择不同 member，但不能回到已在本请求中返回 429 的 member；5xx/transport retry
  保持当前 member，不把 target 故障误判为 key 故障；
- 一个 member 可被并发请求同时使用。第一阶段不实现 reservation、每 key 并发上限、RPM 预测或本地
  token bucket；round-robin 只提供确定性分摊，不承诺严格公平；
- 单 member pool 收到 429 后没有可轮转 key，直接进入现有 fallback/finish 决策，不形成额外 attempt。

### 统一 attempt 与退避

- credential rotation、同 candidate retry 和 Route fallback 全部消耗同一个请求级 `AttemptManager`；继续使用
  6 次请求硬上限、每 candidate 2 次局部上限，并为尚未尝试的 Route 保留预算；pool 大小不得改变上限；
- 任何后续 attempt 前都使用同一 capped exponential backoff：50 ms 起步，逐次翻倍，500 ms 封顶；不因
  “下一个 key 可用”跳过退避，也不叠加第二套 credential backoff timer；
- 429 的 `Retry-After` 只设置失败 member 的跨请求 cooldown deadline：支持 delta-seconds 与 HTTP-date，
  缺失或非法时使用 1 秒，已过期 HTTP-date 视为 0 秒，单次最长 30 秒；当前请求不等待该 member 恢复；
- backoff timer 必须随下游取消而终止；取消后不得选择、借用或调用另一个 credential。

### Cooldown、全池不可用与恢复

- member cooldown 由 `pool id + member binding id + generation` 标识；generation 改变后旧状态不能污染新
  secret；deadline 到期后 member 被动恢复并重新参加 round-robin，不做后台 probe；
- 一个 member 成功不能清除其他 member 的 cooldown，也不能证明共享账号 quota 已恢复；
- 同一请求将 pool 中所有当时可尝试 member 都收到 429，或开始 candidate 时所有 member 都在 cooldown，
  即视为当前 pool 不可用；不得等待最早 deadline 后在同一请求内重新绕一圈；
- pool 不可用时优先进入 RoutePlan 中下一条完整 candidate。若本请求已经获得安全 429 且没有后续 candidate，
  返回最后一个安全 429 与 allowlist `Retry-After`；若没有发起任何 attempt、所有兼容 candidate 都因既有
  cooldown 跳过，则返回稳定的 `503 upstream_cooldown`；
- pool 共享由 pool ID 决定；现有 `quota_scope` 仍用于 target 级边界，但单个 member 的 429 不直接冷却整个
  quota scope。第一阶段不从“一次 429”推断账号级或 Provider 级配额。

### 状态、错误传播与观测

- RoutePlan 在请求开始时继续固定 Target/API/Mode 与 pool binding；实际 member 是 attempt 级选择，不允许
  下游指定，也不改变 capability 或模型选择；
- 多 member pool 不得用于缺少 credential affinity 证明的 `TargetBound` API；无状态 Native/Bridge 请求仍
  遵守原有首输出前边界；
- 多次 credential attempt 后只向下游返回最终安全错误，不公开 pool 大小、member 列表、binding ID、locator
  或 Authorization；
- tracing 可记录非敏感 pool/member binding ID、generation、rotation 原因和 attempt 序号；进程内 metrics
  只增加低基数 `credential_rotations` 计数，不以 pool/member 为 label；
- probe 不自动遍历 pool、不修改 cursor/cooldown，也不把一次 probe 描述为全部 key 可用。逐 key 真实验证
  属于单独、显式执行的 Provider acceptance。

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
- 共享 credential pool/fault domain 会隔离后续无状态请求，target-bound continuation 不因 cooldown 漂移。
- 两个 synthetic credential 的 target 在首个 member 返回 429 后只等待统一 backoff，并以第二个 member 成功；
  后续请求在 cooldown 到期前跳过首个 member，且任何路径都不突破请求/candidate attempt 上限。

已覆盖的测试源码与最近实际运行的验证范围见[当前实现说明](../implementation-status/current-implementation.md)。

## 关联文档

- [网关 API 与客户端兼容](gateway-api-compatibility.md)
- [配置、凭证与受信边界](configuration-and-credentials.md)
- [当前代码架构](../implementation-status/current-architecture.md)
- [当前实现说明](../implementation-status/current-implementation.md)
