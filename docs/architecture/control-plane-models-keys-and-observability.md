# 控制面：模型别名、proxy-issued API key 与可观测性

## 状态

**目标设计，待实施。** 本文定义控制面与转换器的设计边界，不代表当前已有实现；实施任务与退出条件以[开发计划](../plans/development-plan.md)为准。

## 1. 结论

模型聚合、入站授权和日志不是三个独立的 feature：它们共享同一条**不可伪造的 route decision**。

每个可服务请求都应形成不可变 `RouteSnapshot`：

```text
RouteSnapshot
  proxy_request_id
  principal_id
  public_model
  alias_version
  candidate_deployments
  selected_deployment
  protocol_mode: chat | responses | bridge
  capability_decision
  credential_binding_id (id only)
  policy_version
```

它同时用于实际调用、授权检查、响应 header 的安全子集和 audit event。不要让请求日志之后再根据当前配置“猜测”当时调用了什么 route。

## 2. 模型 registry 和别名

### 2.1 数据模型

```text
Provider
  id, type, base_url_allowlist, status

Deployment
  id, provider_id, upstream_model, endpoint_profile
  credential_binding_id
  protocol_capabilities
  health, priority, weight, timeout_policy

PublicModelAlias
  name, version
  candidates: ordered deployment ids
  routing_policy
  advertised_capabilities

PrincipalGrant
  principal_id, alias patterns, allowed endpoints
  requests_per_minute, tokens_per_minute, concurrency_limit
  expiry, audit_policy
```

- `PublicModelAlias.name` 才是 client `model` 字段可以使用的名字；不要把凭证绑定、上游 URL 或 provider-specific model id 暴露为公开路由接口。
- 一个 alias 可以有多个 candidate deployment，但 candidate 是否可用必须同时受 principal grant、protocol mode 和 capability profile 过滤。
- `upstream_model` 是 deployment 内部属性；同一个公开 alias 可以路由到不同 provider 的不同模型，但只有在行为/能力契约明确时才允许。
- `Provider.type` 必须解析为已编译的 Rust `ProviderKind`；配置只描述 route 数据，不能定义任意 header、auth scheme、request/response transform 或 provider 行为。具体 trait 与数据流边界见 [Rust provider adapter 与数据流](rust-provider-adapter-dataflow.md)。

### 2.2 两层 alias，避免把兼容名混成 ACL

区分两个独立映射层：

1. **Global public alias**：`public_name → model group / candidate deployments`，是稳定产品 SKU，例如 `code-primary`；普通调用者可从 `/v1/models` 发现。
2. **Principal-local compatibility alias**：`legacy client name → 已授权 public alias`，仅在该 principal 的 grant 范围内解析；例如为了兼容旧客户端名而映射到一个套餐模型。它不应让 key 获得额外 model group 的权限。

解析顺序必须是：先识别 principal-local alias，再对最终 public alias 做 authorization；不得先按未解析的兼容名放行再重写到未授权 route。LiteLLM 也分别提供 Router 的 model-group alias 与 virtual-key local alias，这证明两个层级不应混为一个全局 map。

### 2.3 解析顺序

```text
request.model
  → exact public alias lookup
  → principal authorization for alias + endpoint
  → protocol / stream / tool / structured-output capability filter
  → health + priority/weight candidate selection
  → credential-binding eligibility check
  → immutable RouteSnapshot
```

不应支持：

- 客户端 `base_url`、`api_key`、`provider`、任意 HTTP header 覆盖 deployment；这会形成 SSRF、credential exfiltration 或策略绕过。
- 模糊匹配把未知模型静默路由到默认 provider；未知 alias 必须返回明确错误。
- 为了 fallback 删除 tools、`response_format`、`background` 或 `previous_response_id` 后继续成功；缺能力应拒绝或发出可检测的转换 notice。

### 2.4 `/v1/models`

第一版的 `/v1/models` 只列出当前 principal 有权调用的 public aliases；返回 OpenAI-compatible 的最小 model objects。它不是 provider capability discovery API，也不应暴露内部 deployment/credential 数量。

控制面更新后使 alias registry version 增加；数据面可短 TTL cache，但一个已开始的 request/stream 始终持有原 snapshot。

LiteLLM 的 virtual key 文档展示了“key 绑定允许模型/alias、并跟踪 spend”的可行产品模型；其 proxy 配置则明确区分 client 看到的 `model_name` 与实际传给 provider 的 model/deployment，Router 提供 priority/health/负载策略。这与本项目的 public alias → deployment 分层一致。本项目只借鉴这一授权和路由边界，不承诺 LiteLLM 的数据库 schema 或管理 API。来源：https://docs.litellm.ai/docs/proxy/virtual_keys、https://docs.litellm.ai/docs/proxy/configs、https://docs.litellm.ai/docs/proxy/load_balancing

## 3. proxy-issued opaque API key

### 3.1 名称与格式

“自签 API key”在本文指 proxy 自行签发的 bearer key；它不是签名 JWT，也不能验证第三方身份。推荐 opaque、可定位但不可从 key id 推导 secret 的格式：

```text
skop_<key_id>_<base64url(random_secret)>
```

- `key_id`：随机、公开可见的 locator，用于查找候选 key record 与审计；不要用 user id/email。
- `random_secret`：CSPRNG 生成，至少 256 bit entropy；只在签发时显示一次。
- key record：`key_id`、principal、secret verifier、allowed grants、created/expiry/revoked/last_used 等，不存明文 secret。

### 3.2 存储和验证

第一版推荐使用 `key_id` 定位记录后，以 **Argon2id** 验证 secret；参数通过 benchmark 选择，并将计算放在不会阻塞 async event loop 的受控 worker/线程池。对极高 QPS，可设计由 KMS/HSM 托管、带 key version 的 keyed-HMAC verifier；这不是把 HMAC key 放进普通应用配置。

验证流程：

1. 严格解析格式和长度；格式错误与未知 `key_id` 走近似相同的失败路径。
2. 从短 TTL key cache 或数据库取得未撤销 record。
3. 使用恒定时间的 verifier compare 验证 secret。
4. 检查过期、principal status、scope/alias/endpoint、RPM/TPM/concurrency。
5. 生成 `AuthenticatedPrincipal`，把原始 bearer key 立即从 request context 移除/标红。

管理面必须支持：签发、显示一次、列出 metadata、撤销、轮换（新旧并存短 grace period）、expiry 和最小 last-used 信息。key rotation 不能依赖修改客户端已有 key 的 secret。

### 3.3 授权不是只有“key 是否有效”

每个 principal 至少有：

- 允许的 public aliases 和 endpoints（Chat / Responses）；
- 请求、token、并发限额；
- 可否使用 streaming、tools、conversion、background/resource API；
- 可否要求内容记录，及对应 audit policy；
- 最大 request body / stream duration（可选）。

先鉴权再路由，或将授权过滤并入 alias candidate selection；绝不能先选择 deployment 再发现 principal 不应调用它，因为这会泄露内部模型/健康状态并可能错误触发上游请求。

## 4. 审计、日志与指标

### 4.1 默认 metadata-only

默认只记录可排障的 metadata；prompt、completion、tool arguments、文件内容和 OAuth material 均不记录。

```json
{
  "event": "proxy.request.completed",
  "timestamp": "...",
  "proxy_request_id": "...",
  "client_request_id": "optional validated external id",
  "principal_id": "...",
  "key_id": "prefix/locator only",
  "endpoint": "/v1/responses",
  "public_model": "code-primary",
  "deployment_id": "...",
  "protocol_mode": "responses",
  "status_code": 200,
  "outcome": "completed",
  "attempt_count": 1,
  "ttft_ms": 0,
  "duration_ms": 0,
  "input_tokens": 0,
  "output_tokens": 0,
  "upstream_request_id": "...",
  "error_class": null,
  "content_capture": "none"
}
```

`proxy_request_id` 必须由 proxy 生成。客户端的 `X-Client-Request-Id` 可作为关联字段，但须限制为 ASCII、长度不超过 512，并不能影响认证、路由或幂等性。OpenAI 的 API Reference 也建议生产环境记录上游 `x-request-id`，并说明 `X-Client-Request-Id` 的格式/长度限制；proxy 应记录两者映射而非覆盖其中任一方。来源：https://platform.openai.com/docs/api-reference/introduction

### 4.2 内容记录是独立的高风险策略

若未来需要请求/响应内容调试，必须同时满足：

- key/principal 被授予 `content_capture` scope；
- 请求显式 opt-in 或有服务端策略；
- 经可测试的 redactor 处理；
- 使用独立访问控制、加密、保留期和删除流程；
- audit event 记录“是否捕获”和 policy version，不在普通 trace 中复制内容。

不要允许任意 client 通过 `no-log=false`、`no-log=true`、debug header 或 callback 名称覆盖服务端的隐私和审计策略。若某类 principal 的合规/计费要求必须审计，应在授权层忽略任何用户可控的禁用日志参数；内容 capture 仍遵循独立的 scope/policy。LiteLLM logging 文档将 correlation id 与 message redaction 分开，这是值得保留的分层；本项目默认更严格，content logging 默认关闭。来源：https://docs.litellm.ai/docs/proxy/logging

### 4.3 SSE 特有指标

流式请求需在 stream 完结或失败后异步写审计，但热路径实时维护：

- accepted time、first upstream byte、first emitted event/token（TTFT）；
- SSE event count / byte count、terminal event kind、terminal response status；
- client cancellation、upstream EOF before terminal、idle timeout、backpressure/slow-client termination；
- conversion notice、dropped/unsupported capability、retry/fallback attempts；
- upstream `x-request-id`、rate-limit headers 的安全摘要。

不能把每个 token 或完整 SSE `data` 同步写数据库。使用有界队列和批量 sink；队列满时明确选择丢弃低优先级 telemetry、阻塞/拒绝请求或降级采样，不能无界内存增长。

## 5. 安全测试矩阵

| 类别 | 必须验证 |
|---|---|
| Key secrecy | 日志/trace/exception/DB dump fixture 中无完整 key、OAuth token、Authorization、cookie |
| Key lifecycle | 创建仅显示一次、正确验证、过期、撤销、rotation grace、cache 失效 |
| Authorization | 无权 alias/endpoint/tool/background/conversion 全部在上游调用前拒绝 |
| Timing and abuse | 格式错误/未知 key/错误 secret 的响应差异受控；rate limit 和 Argon2 不阻塞事件循环 |
| Route safety | client 无法覆盖 URL/provider/credential；alias 不会回退到未授权 deployment |
| Privacy | metadata-only 默认；capture 需要 scope+policy；redaction 处理 JSON、SSE、tool arguments 和 header |
| Stream audit | completed/failed/incomplete/cancel/EOF/timeout 只产生一次 terminal audit event |
| Operations | audit sink 失败、registry reload、credential revoke、部署健康切换均有确定行为 |

## 6. 与 Phase 6 转换器的接口

Protocol converter 不得自行选 provider 或读取 key；它接收的输入是：

```text
AuthenticatedPrincipal
RouteSnapshot
Source wire request/event
CapabilityProfile
```

并返回：

```text
Target wire request/event
ConversionNotice[]
```

这样可以保证：一个 conversion 不能通过悄悄删除 feature 来绕过授权，也不能在日志中丢失原始 public alias、selected deployment 或降级事实。

## 7. 一手参考

- OpenAI API overview（auth、`x-request-id`、`X-Client-Request-Id`、rate-limit headers、向前兼容）：https://platform.openai.com/docs/api-reference/introduction
- LiteLLM virtual keys：https://docs.litellm.ai/docs/proxy/virtual_keys
- LiteLLM logging and redaction：https://docs.litellm.ai/docs/proxy/logging
- OWASP Password Storage Cheat Sheet（Argon2id 参数选择和存储原则）：https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html
- OWASP Logging Cheat Sheet（日志字段与敏感数据处理）：https://cheatsheetseries.owasp.org/cheatsheets/Logging_Cheat_Sheet.html
