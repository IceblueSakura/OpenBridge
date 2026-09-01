# 路由与韧性合同

本文定义固定 Route ordering、attempt、retry/fallback、credential rotation、cooldown、取消及错误边界。

## 固定路由与韧性

本文定义 Public Model 预检后的固定 Route 执行、有限 retry/fallback、单进程短时 cooldown、credential
rotation 与状态亲和边界。已验证的实现范围只见[实施现状](../implementation-status/README.md)。

### 1. Route ordering 的唯一规则

- 下游只选择 Public Model，不得指定 Provider、Upstream Target、Upstream API、endpoint 或 credential。
- 多 Provider 聚合必须由代码目录在一个 Public Model 下显式列出 route source；canonical Model 相同不会自动
  发现、注册或加入 fallback。
- 每个 generation Public Model 必须显式选择一种类型化策略：
  - `NativeFirst`：对每个下游协议，先按 source 声明顺序排列全部 Native Route，再按相同 source 顺序排列
    Bridge Route；
  - `SourceFirst`：对每个下游协议，先保持 source 声明顺序，再在同一 source 内把 Native 排在 Bridge 前。
- 只有当整个 Public Model 缺少某个下游协议的 Native coverage 时，目录才从相反 Native surface 自动补充
  Bridge；显式声明的 Bridge surface 可以在其他 source 已有 Native 时保留。
- 策略编译出的 Route Vec 就是 RoutePlan 的固定配置顺序。运行时不得再次比较 Native/Bridge，也不得按请求
  能力、价格、健康、Provider 名称或模型字符串重新打分、筛选或重排。
- 多 source `gpt-5.6-sol` 以 `SourceFirst` 保持 ChatGPT、OpenAI 顺序；`deepseek-v4-flash` 以
  `SourceFirst` 保持 DeepSeek、Bailian、OpenRouter 顺序。具体可执行候选只由启动注册和 active credential
  收窄，不改变上述 source priority。
- Public Model 的固定能力计算与请求预检由[模型与能力契约](model-capability.md)拥有；本页不为
  单个候选重新计算能力。

### 2. State 与 RoutePlan

- 进入 RoutePlan 的请求已完成一次能力预检；请求能力不得跳过、截断或重排候选。
- RoutePlan 在请求开始后保持固定，不因一次上游响应重新解析 Public Model。
- Public Model 固定接口不公开 `previous_response_id`；非 `null` 值在 RoutePlan 形成前拒绝。上游有状态
  API 是永久非目标，没有可贡献该能力的 executable profile。
- 任何 Provider-bound state 都禁止跨 Target fallback。不能根据 Public Model、canonical model 或 opaque ID
  猜测 issuer。
- `store` 省略或为 `false` 才能进入 planning；`true` 在任何 Provider egress 前拒绝。每个 Responses Native
  candidate 显式编码 `store:false`，Responses-to-Chat Bridge 消费而不伪造 Chat 字段。

### 3. Retry、fallback 与取消

stream/non-stream 请求只可在尚未向下游提交业务 response 时执行有限 attempt。成功 SSE headers 本身不构成提交；首个完整合法且下游可见的 event
仍属于 attempt-owned precommit 边界：

- `429`、明确的 `5xx`、连接失败或 timeout 可按 adapter 分类进入有限 retry；
- Provider 可以在一个物理 attempt 内使用编译期固定的服务端排队 header 缓解突发限流；该等待仍受 Target timeout 和下游取消
  约束，不增加 AttemptCoordinator 计数、不替代 RPM/TPM 配额，也不能由业务请求调整；
- 所有候选共享请求级硬预算，每个候选有独立局部上限，局部 retry 不能无界挤占尚未尝试的候选；
- retry、credential rotation 与 fallback 共享同一 capped exponential backoff，等待必须随下游取消；
- 只有 RoutePlan 允许时才能进入下一条固定候选；本层不重新比较能力或猜测模型等价性；
- 有状态 Responses 永久不存在（有状态 API 是永久非目标），因此没有可进入 Bridge 或跨 Target fallback 的
  stateful 请求；
- 认证失败、无效请求和本地能力拒绝不作为普通 transient failure 重试；
- 一旦向下游提交 response，不得 retry、fallback 或拼接另一上游响应；
- SSE precommit 的 first-event timeout/body transport failure 按现有 transport policy 处理；首个下游 event 前的 invalid framing 或
  terminal 前 EOF 直接完成为安全 502，不自动 retry。第一个下游可见 event 后任何 body error/EOF 均禁止 retry/fallback；
- 下游取消必须终止 pending send、当前上游 stream、backoff timer 与尚未开始的后续 attempt。

统一 attempt contract 固定为请求最多 6 次、每个 candidate 最多 2 次，并为尚未尝试的 Route 保留预算。
backoff 从 50 ms 开始逐次翻倍，500 ms 封顶；credential pool 大小不得改变这些上限。

### 4. Credential pool 与失败分类

| 上游结果 | 是否重试 | 下一 attempt | 跨请求健康作用域 |
|---|---:|---|---|
| HTTP `429`，且尚未提交下游 response | 是 | 同 Target 的下一个可用 credential；无可用 member 时进入既有 fallback/finish | 当前 pool member |
| HTTP `5xx` | 是 | 按 AttemptCoordinator 重试当前 candidate；不因 status 更换 credential | Target 的 `fault_domain` |
| transport failure 或 timeout | 是 | 按当前 candidate retry/fallback；不更换 credential | Target 的 `fault_domain` |
| 非 `429` 的 `4xx` | 否 | 立即返回安全错误 | 不记录 cooldown |
| 已提交下游 response | 否 | 禁止 retry、rotation 或 fallback | 只记录终态 |

- 分类只依赖明确 HTTP/transport 事实，不读取 Provider 私有错误正文猜测 quota；`401/403` 不轮询其他 key，
  避免 credential spraying 和掩盖错误配置。
- 每个 pool 维护并发安全的 round-robin cursor。新 attempt 从 cursor 开始选择第一个未冷却且未被本请求
  `429` 拒绝的 member，选择后推进 cursor。
- 同一请求可在 `429` 后选择不同 member，但不得回到已被本请求拒绝的 member；`5xx`/transport retry 保持
  当前 member。
- member 可以被并发请求共同借用；不提供 reservation、每 key 并发上限、RPM 预测或本地 token bucket，
  round-robin 不承诺严格公平。
- 单 member pool 的 `429` 不制造额外 credential attempt。
- 每次后续 attempt 都执行共享 backoff；`Retry-After` 只决定失败 member 的跨请求 cooldown，不让当前请求
  等待该 member 恢复。

### 5. Cooldown 与恢复

- `429` 只冷却当前 credential member；暂时性 `5xx`、timeout 与 transport failure 冷却 `fault_domain`，未
  显式配置时只隔离 Target。
- `Retry-After` 接受 delta-seconds 与 HTTP-date；缺失或非法时使用 1 秒，已过期 HTTP-date 视为 0 秒，单次
  最长 30 秒。
- 后续无状态请求跳过仍在 cooldown 的 member/fault domain；同一请求的局部 retry 仍受 AttemptCoordinator 上限。
- member cooldown 由 `pool id + member binding id + generation` 标识；credential generation 改变后旧状态
  不得污染新 secret。deadline 到期后被动恢复，不做后台 probe。
- 一个 member 成功只清除自身与当前 Target 的 cooldown，不能清除其他 member 或证明共享账号 quota 已恢复。
- 若所有候选因既有 cooldown 未发起任何 attempt，返回稳定 `503 upstream_cooldown`；若已有安全 `429` 且无
  后续候选，保留最后一个安全 `429` 与 allowlist `Retry-After`。
- 动态权重、后台 probe、持久化健康、跨进程协调、分布式限流和动态 credential 控制面不在本契约内。

### 6. 错误与观测边界

- 下游提交前的最终失败只保留可安全传递的 HTTP status、OpenAI-compatible error 字段、request id、
  `Retry-After` 与 allowlist header；transport timeout/error 使用稳定网关错误。
- 不得转发 credential、Cookie、内部 URL、认证 header、完整堆栈或未经审查的 response header/body。
- 已开始的 SSE 只使用目标协议已有的 error、terminal 或连接关闭语义，不注入私有 event。
- tracing 可以记录非敏感 pool/member binding ID、generation、rotation 原因与 attempt 序号；metric attribute
  不得使用 pool/member 或其他高基数身份。
- probe 不遍历 pool、不修改 round-robin/cooldown，也不把一次探测描述为全部 credential 可用。
