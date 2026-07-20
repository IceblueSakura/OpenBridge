# 开发计划：OpenAI API Proxy

## 状态

**已确认，实施中。** Phase 0 契约基线与 Phase 1 单上游原生转发已完成：Chat/Responses native endpoint、alias/model rewrite、静态下游 Bearer 认证、标准 API-key upstream、共享连接池、下游断开时的上游 stream 取消传播，以及仅限下游尚未收到业务 SSE 的有界 retry/SSE 校验均已落地。其 conformance 覆盖 429/5xx、timeout、EOF、partial-stream failure、断开的 UTF-8、多 event 同 chunk、跨 chunk event 与多行 `data:`；OpenAI Python `2.46.0` 和 Node `6.48.0` SDK 已通过两个端点的 stream/non-stream loopback fixture。Phase 3 已有有序多 deployment candidate、逐 candidate capability gate、受保护的 `/v1/models` 和同协议 streaming fallback；尚未实现多 provider catalog、health/weight 路由策略或 principal 级 alias 过滤。真实 credential store 和 OAuth upstream 仍未完成。

## 1. 目标和已确认决策

构建面向 OpenAI-compatible client 的 proxy，按顺序实现：

1. 原生转发 `POST /v1/chat/completions` 与 `POST /v1/responses`；
2. proxy 自主管理 Codex OAuth 登录和 refresh；
3. 多 provider、deployment、稳定 public model alias；
4. Rust 编译期 provider adapter 与有类型数据流 pipeline；
5. proxy-issued opaque API key、授权与限流；
6. metadata-first 审计、指标和隐私策略；
7. Chat Completions 与 Responses 的双向转换。

### 已确认决策

- **不使用 Codex CLI 作为登录、refresh 或 token storage 的中转。**
- proxy 自己持有、加密保存并 refresh Codex OAuth credential。
- 当前每个 provider 只有**一个 active credential**；不支持同 provider 的多账号、多 workspace、多 credential 路由、轮换池或 failover。
- Phase 1–3 只运行在 loopback 或受信私网；Phase 4 完成入站认证前不得作为共享/public proxy 暴露。
- 先实现 Chat/Responses 的 native forwarding，协议转换放在最后。
- 默认只记录 metadata；prompt、completion、工具参数、API key 与 OAuth material 不进入普通日志。

### 硬门

Codex/ChatGPT OAuth 的真实 client registration、redirect URI、scope/resource、token/refresh endpoint 与适用条款必须先验证。Codex CLI 源码是状态机与安全策略的参考，**不是** proxy 可以长期复用其内部 client ID、私有 endpoint 或未公开协议的授权。

若该硬门不能通过：保留 mock OAuth adapter，Phase 1 仍可使用标准 provider API key；真实 Codex OAuth 接入暂停，而不是导入 `auth.json` 或依赖 Codex CLI 中转。

## 2. 架构边界

Rust 实现采用 [Rust provider adapter 与数据流](../architecture/rust-provider-adapter-dataflow.md) 定义的“编译期 provider catalog + 运行时小型路由配置 + 有类型异步数据流 pipeline”。provider 特定 header、认证规则、请求/响应/SSE/error 映射由 trait 实现表达；配置不能解释任意 provider 行为。

```text
OpenAI-compatible client
  → ingress / request id / body limit
  → proxy key auth + authorization                (Phase 4)
  → public model / alias resolver                 (Phase 3)
  → deployment + capability decision
  → provider adapter
       → active credential for that provider      (Phase 2)
       → configured upstream endpoint
  → native relay or protocol bridge               (Phase 1 / 6)
  → audit / metrics outbox                        (minimum from Phase 0; full in Phase 5)
```

控制面维护 `Provider`、单一 `ProviderCredential`、`Deployment`、`PublicModelAlias`、`ProxyKey` 和审计策略。数据面只使用不可变 route snapshot，不能接收客户端指定的上游 URL、provider credential 或调试配置。

```text
Provider ── 1:1 ── ActiveCredential
                  └─ Deployment(s)

PublicModelAlias ──→ Deployment candidate(s)
ProxyKey ──→ Principal ──→ allowed aliases/endpoints/limits
```

## 3. 分阶段实施

### Phase 0：契约基线、SSE 骨架与 OAuth 可行性 Spike

**目标**：固定可测试边界，不连接真实业务 provider。

**任务**

1. 建立 Rust HTTP service、`cargo fmt`/`clippy`/test baseline、配置加载、`/healthz`、统一 request context 和 OpenAI 风格 error envelope。
2. 定义有类型 dataflow envelope 与 stage trait；按 `ProviderDescriptor`、`RequestAdapter`、`AuthAdapter`、`HeaderAdapter`、`ResponseAdapter`、`ErrorAdapter`、`CapabilityAdapter` 分解 provider 差异。
3. 实现标准 SSE framing：UTF-8 分片、多行 `data:`、注释、空行 event boundary、大小上限与 idle timeout；不得以网络 chunk 作为 JSON/event 边界。
4. 建立 Chat/Responses transcript fixtures，覆盖 text、tool call、usage、unknown event、EOF、timeout、cancel 与错误。
5. 定义 `ProviderCredential`、secret vault interface、`OAuthProviderAdapter` 和 `LoginSession`，但只接 mock issuer。
6. 在 mock issuer 验证 authorization-code + PKCE：state 单次消费、redirect URI、token rotation、并发 refresh、`invalid_grant`、logout。
7. 执行真实 OAuth preflight；不写入或展示真实 token。

**退出条件**

- contract tests 不依赖真实 provider 即能验证协议 parser、错误和 cancellation。
- secret scan 验证 log/exception/fixture 不含 bearer、cookie、API key、access/refresh token。
- service 仅监听 loopback，出站目标严格 allowlist。
- OAuth preflight 得出“可实施”或“阻塞”的明确结论。

### Phase 1：单上游原生转发（已完成）

**目标**：透明支持一个预配置 provider/deployment 的 Chat 与 Responses，不进行模式转换。

**任务**

1. 接收 `POST /v1/chat/completions`、`POST /v1/responses`，校验 method、content type、body limit 和允许的 public model。
2. 实现第一个标准 OpenAI-compatible Rust adapter；使用固定 deployment 调用上游，初期允许标准 API key credential，以避免 OAuth 硬门阻塞 HTTP/SSE 兼容开发。
3. 非流式透明返回 JSON；流式按 SSE event 转发。
4. 生成 proxy `x-request-id`，保留安全的上游 request id、rate-limit header、HTTP status 与错误信息。
5. 在尚未写出下游业务 SSE event 前才允许 retry；已输出部分 stream 后，取消/错误/EOF 按终止语义处理，不重试拼接。
6. client disconnect 时取消上游 HTTP request。

**退出条件**

- OpenAI Python/Node SDK 对两个端点的 stream/non-stream fixture 可消费。
- Chat 保留 `chat.completion.chunk`、`choices[].delta`、terminal `finish_reason`；Responses 保留 JSON `type` 和 response terminal event。
- 429、5xx、timeout、EOF、cancel、断开的 UTF-8 和多 event 同 chunk 可预测处理。
- 不主动发送无 `data:` 的 metadata-only SSE event。

### Phase 2：proxy 自主管理单一 Codex OAuth credential

**目标**：proxy 在不依赖 Codex CLI 的情况下，登录、保存、refresh 与撤销一个 provider 的 active Codex OAuth credential。

**任务**

1. 为每个 provider 建立唯一 active `ProviderCredential`：issuer、account fingerprint、expiry、state、secret reference、secret version、last error；数据库/配置层必须保证 provider 唯一性。
2. 实现浏览器 authorization-code + PKCE login：创建短时 `LoginSession`，生成高熵 state/verifier，callback 一次性消费 state，交换 token，写入 vault。
3. device code 仅在真实 OAuth 契约确认支持且适用后加入；它不是默认首实现。
4. 使用 envelope encryption / secret manager 保存 token；普通 database 仅保存 `secret_ref`、version、expiry、account fingerprint 和状态。
5. 实现状态机：`NeedsLogin → Active → Refreshing → Active | NeedsReauth | Revoked`。
6. 每 provider 一把 refresh lock；refresh 使用 secret version CAS，防止旧 refresh result 覆盖新 token。
7. `invalid_grant`、token reuse、撤销、account/issuer mismatch 进入 `NeedsReauth` 并停止自动 refresh。
8. Codex `AuthAdapter`/`HeaderAdapter` 按 deployment 与 credential binding 构造临时认证头；HTTP handler、logger 和 route resolver 不接触明文 token。
9. 提供最小 admin control plane：credential status、start login、callback、revoke；响应不含任何 secret。

**退出条件**

- 真实 OAuth preflight 已通过；否则此阶段停在 mock adapter，不发布真实 provider。
- browser login → encrypted persistence → restart → request → refresh → revoke 的集成验证通过。
- 并发近过期请求只触发一次 refresh；rotation 具原子性。
- token 不出现在 HTTP response、audit、trace、error、queue、普通 DB 字段或 crash diagnostic。
- provider 的 401 只触发一次受控 refresh/retry，且仅在下游尚未收到业务 stream event 时允许。

### Phase 3：多 provider、稳定 alias 和单 credential 路由（路由基线已完成）

**目标**：聚合多个 provider；每个 provider 仍只对应一个 active credential。

**任务**

1. 扩展 Phase 0 已建立的编译期 `ProviderKind`/`ProviderAdapter` catalog 与 typed route snapshot，加入 `CapabilityProfile`、`RoutingPolicy` 和更多 provider。配置只能选已编译 provider，不能定义 JSON provider 行为或任意 header。
2. public alias 映射到一个 model group/多个 candidate deployments；`/v1/models` 只展示可公开且当前 principal 可访问的 alias。
3. 初期支持 priority、weight、health 和同协议 fallback。
4. capability gate 在上游调用前验证 Chat/Responses、streaming、tools、structured output、background、continuation 等能力。
5. 固定 request 的 route snapshot；一个 stream 期间不因控制面更新而改变 deployment。
6. 不跨 provider replay `previous_response_id`、opaque reasoning、encrypted content 或其他 provider-bound state。

**退出条件**

- 相同 config snapshot 与 request 产生确定 candidate set。
- 未支持 feature/未授权 alias 在上游调用前以明确 4xx 拒绝。
- fallback 记录 candidate、原因、尝试次数与最终 route；不静默删除请求语义。
- 不存在同 provider 多 credential 选择代码或管理接口；未知 provider kind / 任意 provider JSON 规则在配置加载时失败。

### Phase 4：proxy-issued opaque API key、授权与限流

**目标**：让 proxy 可安全服务于受信 client。

**任务**

1. 生成 `skop_<key_id>_<random_secret>`；secret 至少 256-bit entropy，只显示一次。
2. 存储 `key_id`、Argon2id verifier、principal、scope、allowed aliases/endpoints、expiry、revoked、last-used metadata；不保存明文。
3. 实现签发、列出 metadata、撤销、轮换、expiry、短 TTL cache。
4. 在路由前验证 key、principal status、model/endpoint scope、RPM/TPM/concurrency；对错误 key 采用近似失败路径与 abuse protection。
5. 引入 principal/key/IP 限流和管理操作审计。

**退出条件**

- key 不在 log、trace、error 或普通 DB 字段中出现。
- 撤销/过期在 cache 失效预算内生效。
- key 无法访问未授权 model、endpoint、管理 API 或他人 resource。
- 完成后才允许将服务提供给共享网络。

### Phase 5：审计、指标与隐私策略

**目标**：提供可关联的运行证据，默认不留内容。

**任务**

1. 定义结构化 audit event：proxy/client/upstream request id、principal/key locator、public model、deployment/provider、credential state/id、status、error class、retry/fallback、TTFT、duration、usage、terminal outcome。
2. 用有界异步 outbox/queue 写入日志与 metrics，不在 token stream 热路径同步落库。
3. 监控 QPS、4xx/5xx、TTFT、stream duration、cancel、EOF before terminal、retry/fallback、credential refresh、queue pressure。
4. 默认 metadata-only。内容 capture 需独立 scope、服务端 policy、redaction、加密、保留期和访问控制。
5. 不允许 client 使用 `no-log`、debug header 或 callback 参数关闭强制审计。

**退出条件**

- 可用 `proxy_request_id` 关联 ingress、route、upstream 与 terminal event。
- redaction/secret scan 验证 OAuth material、API key、cookie、完整 prompt/response 和工具 secret 不泄露。
- audit sink 故障有明确 fail-open/fail-closed 策略，且队列不会无界增长。

### Phase 6：Chat Completions ↔ Responses 转换

**目标**：在稳定的 native forwarding、capability 和审计基础上实施可检测的协议转换。

**任务**

1. 实现 `wire → Canonical IR → wire` 的 request、final response 和 SSE renderer。
2. 为 Chat 和 Responses 各自维护 stream assembler，追踪 response/item/call/output-index identity、tool argument buffer 与 terminal owner。
3. 加入 re-entry guard，避免 bridge 递归选择另一 bridge。
4. 对 built-in tools、background/resource APIs、`previous_response_id`、opaque reasoning、status 等不等价能力返回明确错误或 `ConversionNotice`。
5. 不执行有副作用的 tool call；仅保持 wire-level conversion。

**退出条件**

- 文本、并行 function calls、tool output、usage、structured output、cancel、error、EOF fixture 通过。
- `output_item.done` 不提前结束 Responses stream；tool arguments 仅在完整后 parse/validate。
- 每个有损转换都有 machine-readable notice 和 audit record。

## 4. 质量门

| 层次 | 最小验证 | 阶段 |
|---|---|---|
| Unit | key verifier、credential state、refresh CAS、alias resolver、SSE parser | 0–6 |
| Fixture/contract | Chat/Responses JSON、SSE transcript、未知字段、EOF、cancel | 0、1、6 |
| Mock integration | OAuth issuer、provider HTTP、retry、cancellation、admin credential API | 0–2 |
| SDK compatibility | OpenAI Python/Node 调用 proxy | 1、6 |
| Security | secret scan、authorization matrix、SSRF allowlist、redaction、rate limit | 0、2、4、5 |
| Load/soak | concurrent streams、slow consumers、refresh storm、audit backpressure | 2、4、5、6 |

## 5. 当前非目标与延期项

- 同 provider 多凭证、多账户、多 workspace 路由与 credential load balancing。
- 导入、复制或要求上传 Codex `auth.json`。
- 把 proxy 作为 ChatGPT OAuth issuer、OAuth MITM 或 auth endpoint 的替代品。
- 全部 OpenAI resources、Realtime、Files、Conversations 和管理 API。
- 默认记录完整 prompt/response。
- 对协议转换承诺无损语义或支持所有 Responses resource/background 生命周期。

## 6. 主要风险与退出策略

| 风险 | 处理 |
|---|---|
| 真实 OAuth 无公开适用契约或不允许自建 client | 不接入真实 Codex OAuth；停在 mock adapter，使用标准 API-key upstream |
| refresh token rotation 竞态 | 每 provider 单飞 lock + secret version CAS + `NeedsReauth` 终态 |
| token 泄露 | vault、最小内存暴露、redaction、secret scan、禁止普通日志/trace/queue 持有 bearer |
| 上游协议漂移 | 固定 fixture、SDK compatibility test、provider adapter 隔离、版本化 capability profile |
| 跨 provider 状态错误重放 | route snapshot + provider-bound state affinity；不跨 provider fallback opaque state |
| shared proxy 被滥用 | Phase 4 前不公开；opaque key、scope、rate/concurrency limit 和审计后再开放 |

## 7. 证据来源

- 用户确认：proxy 自主管理 OAuth login/refresh；不使用 Codex CLI；当前每 provider 仅一个 credential。
- [Rust provider adapter 与数据流](../architecture/rust-provider-adapter-dataflow.md)：Rust trait adapter、编译期 provider catalog、数据流 pipeline、配置边界与性能门。
- [架构与路线](../architecture/architecture-and-roadmap.md)：HTTP/SSE、路由、key 与 observability 方案。
- [Codex OAuth 凭证边界](../design/codex-oauth-credential-boundary.md)：Codex OAuth 参考实现、credential lifecycle 与安全边界。
- [控制面、模型、密钥与可观测性](../architecture/control-plane-models-keys-and-observability.md)：alias、proxy key、审计策略。
- OpenAI Codex auth：https://developers.openai.com/codex/auth
- OpenAI API streaming：https://platform.openai.com/docs/guides/streaming-responses
- OAuth 2.0 Security BCP：https://datatracker.ietf.org/doc/html/rfc9700
- PKCE：https://datatracker.ietf.org/doc/html/rfc7636
