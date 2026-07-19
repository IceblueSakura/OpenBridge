# OpenAI API Proxy：目标架构与分阶段路线

## 状态

本文定义目标架构、阶段边界和验收意图。具体实施任务与退出条件以[开发计划](../plans/development-plan.md)为准；两者出现表述差异时，以开发计划的可执行定义优先。

## 1. 结论

推荐把 proxy 拆为**数据面**和**控制面**：数据面只负责已授权请求的解析、路由、转发、SSE/协议转换和最小审计；控制面管理上游凭证、deployment、模型别名、proxy key、策略和日志投递。不要让 HTTP handler 同时承担 OAuth、密钥存储、模型管理和协议转换。

```mermaid
flowchart LR
    C[OpenAI-compatible client] --> I[Ingress: request id / size limit]
    I --> A[Inbound auth + authorization]
    A --> V[Protocol validation]
    V --> M[Public model resolver]
    M --> R[Route selector + capability gate]
    R --> P[Native forwarder or protocol bridge]
    P --> U[Provider adapter]
    U --> X[Upstream provider]
    X --> U --> P --> C

    K[Key control] --> A
    D[Deployment / alias registry] --> M
    D --> R
    S[Credential vault] --> U
    L[Audit / metrics sink] <-- I
    L <-- A
    L <-- R
    L <-- P
```

## 2. 边界和不变量

### 2.1 数据面

数据面是请求热路径，输入只有 HTTP request、已验证 principal 和不可变的 route snapshot。它必须：

- 不读取明文上游 credential；只按 credential binding id 向受控 provider client 取临时认证头。
- 在首次对外可用前即生成 `proxy_request_id`；将客户端 `X-Client-Request-Id` 作为可记录的外部关联值，而不是授权依据。
- 对请求 body、SSE event 和缓冲区设置上限；按协议而非网络 chunk 解析 SSE。
- 固定 `model → alias → deployment candidates` 的解析结果，避免同一 stream 中控制面配置改变导致路由漂移。
- 不信任或透传客户端提供的 provider、`api_base`、proxy 配置、上游 credential、内部 metadata 和调试 header。
- Rust provider 扩展通过 [Rust provider adapter 与数据流](rust-provider-adapter-dataflow.md) 定义的 trait 组合注入；数据流 stage 不按 provider name 写分支，也不解释运行时 JSON Registry 规则。

### 2.2 控制面

控制面管理慢变化配置和敏感状态：

```text
Provider
  └─ CredentialBinding (secret reference, issuer/account/expiry/status)
       └─ Deployment (upstream endpoint + upstream model + capability profile)
            └─ PublicModelAlias (stable name + routing policy)

Principal / ProxyKey
  └─ grants: allowed aliases, endpoints, RPM/TPM, expiry, audit policy
```

控制面写入必须有 audit event；数据面只读已验证、带版本的 snapshot。第一版可以是受版本控制的本地配置和单进程内存 cache，不要在模型路由尚未稳定前先建设完整管理 UI。

### 2.3 两类认证绝不混用

| 认证对象 | 作用 | 可出现的位置 | 禁止事项 |
|---|---|---|---|
| 入站 proxy key | 识别和限制调用 proxy 的 client | 标准 HTTP bearer header；仅在入站校验层使用 | 不能作为上游 OpenAI/Codex credential |
| 上游 provider credential | 代表 proxy 访问一个 provider/deployment | credential vault → provider adapter | 不返回给 client；不入请求/响应日志 |
| Codex/ChatGPT OAuth token | 特定 issuer/account 的上游凭证或 Codex client 认证材料 | 仅专用 credential adapter | 不跨账户、issuer、endpoint 或 provider replay |

## 3. 分阶段路线

用户的六项目标保留，但需要增加 Phase 0，并将“可公开运行”的安全门前移。Phase 1–3 只能绑定 loopback 或私有开发网络；Phase 4 完成前不得作为共享 proxy 暴露。

### Phase 0：契约基线和可替换骨架

**目标**：在真正转发前固定可测试的公共边界。

**范围**

- 创建 request context、统一错误模型、provider client interface、route snapshot、SSE parser interface。
- 导入 Chat/Responses 非流式与流式 fixtures；标记 native-pass-through 和 conversion 两种期望。
- 建立 `/healthz` 与只读配置版本诊断；不泄露 deployment URL 或 credential 状态细节。
- 实现 request size、stream event size、idle timeout、取消传播和 outbound allowlist 的基础设施。

**验收门**

- 不连真实 provider 的 contract test 可验证解析、错误和 SSE framing。
- 任何日志/异常中不存在 `Authorization`、cookie、OAuth token、refresh token 或完整 API key。
- 仅监听 loopback；配置不能让请求指定任意出站 URL。

### Phase 1：Chat 与 Responses 的原生转发

**目标**：支持 `POST /v1/chat/completions` 和 `POST /v1/responses` 到一个已配置上游 deployment 的透明转发；不做模式转换。

**范围**

- 验证 HTTP method/content type/body 上限，并只接受配置允许的 public model。
- 将 public model 映射成一个预置 deployment；转发允许的请求字段，保留 upstream HTTP status、`openai-request-id`、rate-limit headers 和错误 body 的安全子集，同时保留 proxy 自己的 `x-request-id`。
- 非流式 JSON 透明返回；流式按 SSE event 转发，不能将网络 chunk 当 event/JSON 边界。
- 客户端取消必须取消上游 HTTP request；EOF 未见协议终态必须记录为不完整 stream，而不是伪造成功。
- 每个成功、失败和 SSE 请求生成稳定的 proxy `x-request-id`；在尚未写出下游 SSE bytes 前，使用 OpenAI 风格 JSON error envelope 返回失败，并保留安全的 `retry-after` / `x-should-retry` 语义。
- 上游 retry 仅可发生在尚未向下游发出业务 SSE event 前；已输出部分 stream 后不得重试并拼接第二次结果。

**明确不做**

- 多 provider fallback、动态别名、OAuth login、公开 API key、Chat ↔ Responses 转换、Responses resource emulation。

**验收门**

- OpenAI SDK Python/Node 的 Chat/Responses 非流式与 `stream=true` fixture 均可消费。
- Chat 流保留 `choices[].delta` 与终态 `finish_reason`；Responses 流保留 JSON `type` 和 `response.completed/failed/incomplete`。
- 用断开的 UTF-8、多 SSE event 同 chunk、一个 event 跨多个 chunk、多行 `data:`、取消和 idle timeout fixtures 验证。

**SDK 兼容性注意**：SSE 标准允许只含 `id`/`retry`/`event` 的 metadata frame，但当前 OpenAI Python SDK 的 stream loop 会对其 decoder 产出的每个 event 调用 JSON decode；首版 proxy 不应主动生成无 `data` 的 metadata-only frame。此限制是 SDK 互操作策略，不是把 metadata frame 误判为非法协议。已核对源码快照 `openai/openai-python@d4dceb221b9a92c55c232d5b330ae89beb539415`：[stream loop](https://github.com/openai/openai-python/blob/d4dceb221b9a92c55c232d5b330ae89beb539415/src/openai/_streaming.py#L57-L101)、[SSE field decoder](https://github.com/openai/openai-python/blob/d4dceb221b9a92c55c232d5b330ae89beb539415/src/openai/_streaming.py#L333-L386)。

### Phase 2：proxy 自主管理单一 Codex OAuth credential

**目标**：不通过 Codex CLI 中转；proxy 自己完成 provider 的 browser PKCE login、加密保存、refresh、revoke 和 re-login。当前每个 provider 只允许一个 active credential；详见 [Codex OAuth 凭证边界](../design/codex-oauth-credential-boundary.md) 与 [开发计划](../plans/development-plan.md)。

**范围**

- 建立 `ProviderCredential`、secret vault、短时 `LoginSession` 和 provider adapter；普通数据库只保存 secret reference/version/expiry/account fingerprint/state。
- 先实现 browser authorization-code + PKCE；device code 仅在真实 OAuth 契约确认支持且适用后加入。
- 每 provider 单飞 refresh lock + secret version CAS；`invalid_grant`、token reuse、撤销或 issuer/account mismatch 进入 `NeedsReauth`。
- provider adapter 仅向配置 allowlist deployment 临时构造认证头；数据面、日志、route resolver 不接触明文 token。
- 提供受信 admin login/callback/status/revoke 接口；不导入 `auth.json`，不使用 CLI 作为 token relay。

**验收门**

- mock issuer 验证 PKCE/state、redirect/issuer substitution、rotation、并发 refresh、`invalid_grant`、revoke 和 restart。
- browser login → encrypted persistence → request → refresh → revoke 的真实链路只有在 OAuth client registration、endpoint、scope/resource、条款 preflight 通过后实现。
- token 不出现在 HTTP response、audit、trace、error、queue、普通 DB 字段或 crash diagnostic。
- 若真实 OAuth preflight 不通过，真实 Codex OAuth 接入停止在 mock adapter；Phase 1 的标准 API-key upstream 路径保持可用。

### Phase 3：多 provider、deployment 和稳定模型别名

**目标**：一个 public model 可解析为多个同能力 deployment，并在明确策略下选择上游。

**范围**

- 静态 registry：`Provider`、`Deployment`、`PublicModelAlias`、`CapabilityProfile`、`RoutingPolicy`；当前每个 provider 只绑定一个 active credential。
- 只读 `/v1/models` 从 public aliases 生成；不暴露上游 credential 或内部 provider route。
- alias 解析、权限过滤、健康状态、优先级/权重、有限的同协议 fallback。
- 每个 deployment 标注 `chat/responses`、streaming、tool kinds、structured output、background、continuation replay 等能力。

**验收门**

- alias 解析是确定性的：相同 snapshot + principal + request 产生相同 candidate set。
- 无能力的参数在调用前返回可解释的 4xx/`unsupported_feature`，不会静默删除。
- fallback 不跨 protocol、不跨 issuer replay opaque state，且把尝试次数/最终 route 写入审计。

### Phase 4：入站 proxy-issued API key 和授权

**目标**：让 proxy 可安全供受信 client 使用；详见 [控制面、模型、密钥与可观测性](control-plane-models-keys-and-observability.md)。

**范围**

- 签发、显示一次、验证、撤销、过期、轮换 proxy-issued opaque key。
- principal → aliases/endpoints/scopes/RPM/TPM/concurrency/日志策略的授权决策。
- auth 失败的恒定时间比较、最小错误信息、IP/principal 限流、防暴力尝试和管理操作审计。

**验收门**

- 数据库和日志均没有明文 key；撤销和过期在 cache 失效预算内生效。
- 一个 key 不能访问未授权 alias、管理 API、未授权 endpoint 或其他 principal 的 resource。
- 负载测试确认 Argon2/verification 与 rate limiter 不会阻塞 SSE/请求事件循环。

### Phase 5：请求审计、指标和隐私策略

**目标**：可关联、可排障、默认不记录 prompt/response 内容的观测能力；详见 [控制面、模型、密钥与可观测性](control-plane-models-keys-and-observability.md)。

**范围**

- 结构化 audit event、metrics、trace correlation、SSE terminal/TTFT/耗时/usage/route/result 分类。
- 默认 metadata-only；内容捕获必须是显式、受权限控制、有保留期和 redaction 的独立策略。
- 日志入队异步化，避免在 token stream 热路径同步写数据库。

**验收门**

- 能通过 `proxy_request_id` 关联入站、路由、上游 `x-request-id`、最终状态和重试。
- redaction 测试证明不会记录 bearer token、cookie、OAuth material、完整 API key、工具 secret 或未授权内容。
- audit sink 不可用时有明确 fail-open/fail-closed policy，且不会无界占用内存。

### Phase 6：Chat Completions ↔ Responses 转换

**目标**：在明确 capabilities 和可观测的降级策略下实现双向转换。

**前置条件**：Phase 1 的 native forwarding、Phase 3 capability profile、Phase 5 observability 都已完成。

**范围**

- 实现 `wire → Canonical IR → wire` 的双向 request、final response 和 SSE renderer；Canonical IR 详见 [Chat/Responses 转换设计](../design/chat-responses-conversion.md)。
- 两个独立 stream assembler；维护 item/call/response/output-index identity 和 terminal owner。
- re-entry guard，防止 Responses→Chat fallback 又被再次选择为 Chat→Responses bridge。
- 对 built-in tools、background/resource APIs、`previous_response_id`、opaque reasoning 和 status 的不等价情况返回明确 capability error 或 conversion notice。

**验收门**

- Chat↔Responses 的文本、并行 function calls、tool result、usage、structured output、stream terminal/error/cancel fixtures 通过。
- `output_item.done` 不会结束 Responses stream；tool arguments 只在完整后 parse/validate。
- 每个有损转换都有 machine-readable `ConversionNotice` 和审计记录；不得静默伪造 native lifecycle。

## 4. 质量门和测试层次

| 层次 | 最小验证 | 适用阶段 |
|---|---|---|
| Unit | validation、alias resolver、key verifier、token state machine、SSE parser | 全部 |
| Fixture/contract | 官方 Chat/Responses JSON 与 SSE transcript；断流/错误/未知字段 | 0、1、6 |
| Mock provider integration | HTTP headers、cancellation、retry、OAuth refresh、route fallback | 1–4 |
| SDK compatibility | OpenAI Python/Node SDK 调用 proxy 的非流式/流式场景 | 1、6 |
| Security | secret scan、authorization matrix、redaction、SSRF/URL allowlist、rate limit | 0、2、4、5 |
| Soak/load | 并发 SSE、slow consumer、credential refresh storm、audit sink backpressure | 2、4、5、6 |

## 5. 已否决或延期的方案

- **先做转换后做 native forwarding**：否决。缺少透明基线时无法定位问题来自 adapter、provider 还是 converter。
- **把所有 provider 都伪装成原生 Responses**：否决。Responses 有 resource、background、built-in tool 和 continuation 语义；不能只返回一个 JSON 外形。
- **将 OAuth token 当作客户端 API key**：否决。生命周期、授权边界、撤销和审计语义不同，且扩大泄露面。
- **在 Phase 4 前公网开放**：否决。未认证转发会成为共享上游 credential 的滥用通道。
- **默认记录完整 prompts/responses 以便调试**：否决。先记录 metadata 和关联 id；内容记录只作为受控诊断能力。

## 6. 一手参考

- OpenAI API overview（authentication、request IDs、rate limit headers、向前兼容）：https://platform.openai.com/docs/api-reference/introduction
- OpenAI streaming guide（Responses typed events 与 Chat data-only chunks）：https://platform.openai.com/docs/guides/streaming-responses
- OpenAI Responses streaming event reference：https://platform.openai.com/docs/api-reference/responses-streaming
- LiteLLM virtual keys（模型范围、key 生命周期、spend tracking 的参考）：https://docs.litellm.ai/docs/proxy/virtual_keys
- LiteLLM logging（correlation id、metadata-only/redaction 的参考）：https://docs.litellm.ai/docs/proxy/logging
- OpenAI Python SDK（SSE decoder、retry/error contract 的实现参考）：https://github.com/openai/openai-python
- OpenAI Node SDK（SSE decoder、retry/error contract 的实现参考）：https://github.com/openai/openai-node
