# Codex Responses HTTP / WebSocket Header 行为基线

## 状态与证据

| 项目 | 值 |
|---|---|
| 调研仓库 | openai/codex |
| 固定上游提交 | 44b857c00e5803adedbc5b2e94c4a33574a157fe |
| 复核日期 | 2026-09-22 |
| 主要范围 | codex-rs/core 与 codex-rs/codex-api 中的 Responses HTTP/SSE、Responses WebSocket、ChatGPT OAuth 模型调用路径 |
| 排除 | 登录页、GitHub、Cloud Tasks、遥测上传等与模型 Responses 调用无直接关系的 HTTP 客户端 |

本文维护当前 Codex 客户端在模型 Responses 调用路径中显式构造、消费或兼容投影的 HTTP Header，以及这些 Header 的生命周期、来源和主要用途。

“全部 Header”在本文中的含义是：固定快照中由 Codex 模型调用路径明确拥有的 Header，以及该路径明确解析的响应 Header 家族。ModelProviderInfo.http_headers、env_http_headers、CodexResponsesHeaders 等允许宿主或 Provider 注入任意附加 Header，因此其具体名称天然不可穷举；本文单独记录这些扩展入口，不把它们误写成固定 Codex 协议。

本文是外部实现调研，不表示这些私有 Header 是公开 OpenAI Responses API 的通用合同。第三方网关只能在明确 Provider profile 中采用相应行为。

---

## 1. 核心结论

当前 Codex 的 Responses 请求上下文已经不是“一组散落的 session Header”，而是多个不同生命周期的执行状态：

1. logical session、cache affinity、thread identity、context window、turn sticky state 是不同概念。
2. ChatGPT Responses 的 session-id 对 root agent 主要承载 cache affinity；它不必等于真实 logical session id。
3. thread-id 与 x-client-request-id 当前都跟随 Codex thread identity；x-client-request-id 不是每次 HTTP 请求重新生成的随机 request id。
4. x-codex-turn-state 是服务器签发的 turn-scoped opaque token。客户端只能接收、同 turn 原样重放、跨 turn 清除，不能自行派生。
5. x-codex-routing-hint 表达 model / service tier 路由提示，不是 sticky token。
6. x-codex-turn-metadata 及 Responses body 的 client_metadata 承担越来越多的 canonical 执行上下文；直接 HTTP/WS Header 是其中一部分兼容投影。
7. Authorization、ChatGPT-Account-ID、X-OpenAI-Fedramp 属于 credential / account routing 边界；不得与普通 session/cache metadata 混为一类。
8. HTTP 与 WebSocket 尽量共享会话、线程和兼容 metadata，但 WebSocket 还存在 handshake-only Header 与 message-level client_metadata。

---

## 2. 生命周期模型

| 生命周期 | 典型信息 | 说明 |
|---|---|---|
| Account / credential | Authorization、ChatGPT-Account-ID、X-OpenAI-Fedramp | 绑定认证主体、账号和合规边缘 |
| Installation | installation_id | 主要进入 client_metadata / turn metadata；当前普通 Responses 请求不是直接 HTTP Header |
| Session / cache scope | logical session_id、prompt_cache_key、HTTP session-id | logical session 与 cache affinity 可不同 |
| Thread | thread_id、thread-id、x-client-request-id | Codex thread 稳定身份 |
| Context window | window_id、x-codex-window-id | 同一 thread 在 compaction 后可进入新的 window |
| Turn | turn_id、x-codex-turn-state | turn-state 由服务器签发，仅在同一 turn 重放 |
| Request / operation | x-openai-subagent、request_kind、x-codex-routing-hint | 描述本次执行类型、agent 来源、model/tier |
| Transport | Accept、OpenAI-Beta、压缩相关 Header | HTTP/SSE 或 WebSocket 协商 |

这一分层非常重要：不能生成一个 UUID，然后同时作为 session-id、thread-id、x-client-request-id 和 x-codex-turn-state 使用。

---

## 3. 请求 Header 总表

### 3.1 认证、客户端身份与合规

| Header | 来源 / 值规则 | 生命周期 | 主要用途 | 行为边界 |
|---|---|---|---|---|
| Authorization | AuthProvider 写入 Bearer token | credential | API / ChatGPT OAuth 认证 | 敏感凭据；最后绑定，不接受普通执行 metadata 覆盖 |
| ChatGPT-Account-ID | ChatGPT bearer auth 中的 account_id | account/workspace | 将 bearer 绑定到 ChatGPT account/workspace | ChatGPT subscription 路径的重要认证上下文 |
| X-OpenAI-Fedramp | FedRAMP account 时固定 true | account | 选择 FedRAMP routing/compliance edge | 只由认证上下文决定 |
| originator | 默认客户端身份为 codex_cli_rs；thread 可有合法 override | session/client | product attribution、后端兼容与遥测 | 不是 session/cache key |
| User-Agent | Codex 版本与平台信息 | process/client | 客户端版本识别、兼容与诊断 | 默认 HTTP client Header |
| x-openai-internal-codex-residency | managed residency 为 US 时值 us | managed configuration | 数据驻留 / routing policy | managed policy 可覆盖普通 provider header |
| x-oai-attestation | AttestationProvider 按 thread 即时生成 | request/thread context | 受支持环境中的设备/宿主 attestation | 只有 provider/profile 要求时发送 |

### 3.2 Session、cache、thread 与 turn

| Header | 来源 / 值规则 | 生命周期 | 主要用途 | 关键行为 |
|---|---|---|---|---|
| session-id | ModelClient.responses_session_id() | session/cache scope | ChatGPT Responses cache affinity / backend affinity | root agent 通常使用 effective prompt_cache_key；non-root agent 使用真实 metadata.session_id |
| thread-id | responses_metadata.thread_id | thread | thread identity / backend correlation | 同一 Codex thread 稳定 |
| x-client-request-id | 当前直接写入 responses_metadata.thread_id | thread | 客户端请求关联 / tracing correlation | 当前不是每次请求唯一随机值 |
| x-codex-turn-state | 首次由服务器响应返回，客户端同 turn 重放 | turn | sticky routing | OnceLock 保存；新 turn 不得重放；auth ownership 变化时清除 |
| x-codex-window-id | responses_metadata.window_id | context window | context-window identity | 与 thread identity 分离；compaction 后可变化 |
| x-codex-parent-thread-id | parent_thread_id 存在时投影 | agent lineage | parent/child thread 关联 | 只在有明确 lineage 时发送 |
| x-codex-turn-metadata | CodexResponsesMetadata 生成 bounded JSON | turn/request | 完整 Codex 执行上下文的兼容 Header 投影 | canonical 事实更偏向 client_metadata 中同名字段；Header 为兼容输出 |

注意：x-codex-installation-id 虽然存在常量和 client_metadata key，但当前普通 Responses model path 的 compatibility_headers() 不把它直接投影为 HTTP Header；它主要存在于 client_metadata / turn metadata 中。

### 3.3 路由、功能与 Agent 类型

| Header | 来源 / 值规则 | 生命周期 | 主要用途 | 关键行为 |
|---|---|---|---|---|
| x-codex-routing-hint | model=<model> 或 model=<model>;tier=<service_tier> | request/connection | backend pool / model-tier routing hint | 不是 sticky state；Guardian reviewer 等特殊路径可不发送 |
| x-openai-subagent | review、compact、memory_consolidation、collab_spawn 或明确 label | request/session source | 标识子代理 / 内部 worker 类型 | 普通 CLI/VSCode/Exec 等 root source 通常不发送 |
| x-openai-memgen-request | MemoryConsolidation internal session 时 true | request | 标识 memory generation / consolidation | 只用于对应内部请求 |
| x-codex-beta-features | session 启用的 beta feature 列表 | session/request | 后端 beta feature gating | 空值不发送 |
| x-openai-internal-codex-responses-lite | use_responses_lite 时 true | request | 选择 Codex Responses Lite 行为 | 私有功能 Header |
| x-responsesapi-include-timing-metrics | include_timing_metrics 时 true | WS handshake / request profile | 请求服务端返回 timing metrics | 当前 WebSocket handshake 明确发送 |
| OpenAI-Beta | WebSocket v2 固定 responses_websockets=2026-02-06 | WS connection | Responses WebSocket 协议版本协商 | WebSocket handshake 专用 |
| Accept | HTTP Responses 固定 text/event-stream | HTTP request | 要求 SSE 流 | 普通 HTTP streaming Responses 路径 |
| Content-Type | JSON request 由 HTTP client / encoder 设置 | request | JSON body | 标准 transport Header，不属于 Codex 私有 metadata |
| Content-Encoding | 开启 request compression 时由 transport 设置 | request | 请求体压缩 | 属于 transport 能力，不应进入 semantic/session context |

### 3.4 宿主与 Provider 可扩展 Header

两类入口允许增加固定表之外的 Header：

1. ModelProviderInfo.http_headers / env_http_headers：Provider 配置级 Header，可用于 custom provider 或特殊认证/路由。
2. CodexResponsesHeaders：host-created thread 可注入 model-scoped Responses Header，只在匹配模型、Codex backend auth 与 routing 条件下应用；HTTP 请求和 WebSocket handshake 都会使用，其他 endpoint 和 thread 不继承。

因此 x-codex-guardian 等 extension Header 可以存在于特定产品路径，但它们不是所有 Codex Responses 请求的固定 Header。

---

## 4. prompt_cache_key 与 session-id 的精确规则

Codex 当前把 Responses body 的 prompt_cache_key 和 ChatGPT HTTP session-id 明确联系起来。

### 4.1 effective prompt cache key

ModelClient.prompt_cache_key() 的优先级为：

    explicit prompt_cache_key override
        > Internal session + parent_thread_id 时使用 "<source>:<parent_thread_id>"
        > responses_metadata.session_id

build_responses_request() 会把这个 effective value 写入 Responses body：

    prompt_cache_key = effective_prompt_cache_key

### 4.2 HTTP session-id

ModelClient.responses_session_id() 的规则为：

    non-root agent:
        session-id = responses_metadata.session_id

    root agent:
        session-id = effective_prompt_cache_key

源码注释直接说明：ChatGPT 从 Responses session-id 派生 cache affinity，而真实 session identity 继续保存在 turn metadata、hook、history/notes 等上下文。

因此 root agent 的常见形态为：

    logical session identity = S
    effective cache affinity = K

    body.prompt_cache_key = K
    HTTP session-id        = K
    client_metadata.session_id / turn metadata = S

这证明 session-id 在 ChatGPT wire 上不能简单等同于 OpenBridge 内部的 LogicalSessionId。

### 4.3 对 OpenBridge 的含义

OpenBridge 如果从稳定 prompt prefix 派生 Gateway CacheAffinityKey K，可以在 ChatGPT Provider profile 中映射：

    prompt_cache_key = K
    session-id       = K

但内部仍应把 logical session、cache affinity、thread identity 分开。没有真实 thread/session 语义来源时，不应为了“补齐 Codex Header”伪造 thread-id、parent-thread-id 或 turn-state。

---

## 5. thread-id 与 x-client-request-id

Responses HTTP client 当前执行：

    if thread_id exists:
        x-client-request-id = thread_id

    session-id = ...
    thread-id  = thread_id

因此：

- thread-id 是 Codex thread 的稳定身份；
- x-client-request-id 当前复用相同 thread_id；
- 不能根据 Header 名字推断 x-client-request-id 是“每个 HTTP attempt 唯一”的 request UUID；
- 真正的服务端 request correlation 还会通过响应 x-request-id / x-oai-request-id 等返回。

OpenBridge 若需要自己的 per-request RequestIdentity，应独立维护，不要默认映射成 Codex x-client-request-id。

---

## 6. x-codex-turn-state：服务器签发的 turn sticky state

这是生命周期最严格的私有状态。

ModelClientSession 每个 Codex turn 新建一个 Arc<OnceLock<String>>：

1. turn 的首个 Responses 请求没有客户端自行制造的 turn-state；
2. HTTP/SSE response 或 WebSocket response/metadata 提供 x-codex-turn-state；
3. client 将首个合法值保存到 OnceLock；
4. 同一个 turn 的 retry、incremental append、continuation 等后续请求原样重放；
5. 新 turn 创建新的 ModelClientSession，不能继承上一 turn token；
6. auth ownership 变化时相关 connection/turn routing state 被重置。

源码注释明确指出跨 turn 重放会违反 client/server contract 并可能产生 routing bug。

因此 OpenBridge 对它只有三种安全策略：

- transparent proxy：上下游真正的 Codex runtime 自己管理，Gateway 双向透传；
- managed turn runtime：Gateway 确实拥有 turn 生命周期时，保存并按 owning turn 重放；
- unsupported：没有 turn owner 时不生成、不猜测。

绝不能从 session id、cache key 或 prompt hash 派生 x-codex-turn-state。

---

## 7. Canonical turn metadata 与 Header 兼容投影

CodexResponsesMetadata 的源码注释已经明确：

> 完整 Codex turn metadata 的 canonical transport 是 client_metadata["x-codex-turn-metadata"]；flat client_metadata key 与直接 HTTP/WS Header 是这个 snapshot 的 compatibility projection，而不是多个独立事实源。

当前 metadata 可包含：

- installation_id、session_id、thread_id、agent_name、turn_id；
- window_id、window_number、context_window_id；
- request_kind：turn / prewarm / compaction / memory；
- forked_from_thread_id、parent_thread_id、parent_turn_id、root_turn_id；
- subagent_kind、thread_source、turn_trigger；
- sandbox、sandbox_mode；
- auto-review / node-repl 状态；
- workspace 摘要；
- tool namespace 信息；
- history ingest、analytics、compaction metadata；
- bounded extra metadata。

### 7.1 client_metadata 的直接投影

普通 client_metadata 至少建立以下 key：

- x-codex-installation-id
- session_id
- thread_id
- x-codex-window-id
- 可选 turn_id
- 可选 x-openai-subagent
- 可选 x-codex-parent-thread-id
- 可选 parent_turn_id / root_turn_id
- 有 turn metadata 时的 x-codex-turn-metadata

### 7.2 HTTP / WS compatibility_headers()

直接 Header 只投影：

- x-codex-window-id
- x-codex-turn-metadata
- x-codex-parent-thread-id
- x-openai-subagent

其中 x-codex-turn-metadata Header 为 bounded compatibility form，不携带完整 tool namespace inventory；完整 metadata 保留在 body client_metadata。

这意味着 OpenBridge 不应把每一个 x-codex-* 名称都建模成独立的 canonical semantic field。更合理的是拥有 provider-neutral execution context，再由 ChatGPT/Codex profile 决定是否需要兼容 Header 投影。

---

## 8. HTTP 与 WebSocket 的差异

### 8.1 HTTP / SSE

Responses HTTP 请求：

- body 为 Responses request；
- Accept: text/event-stream；
- session-id、thread-id、x-client-request-id 作为 Header；
- compatibility headers、originator、attestation、beta/features 等按条件加入；
- x-codex-turn-state 在同 turn 后续请求作为 Header 重放；
- server 首部和 SSE body 分开处理。

### 8.2 WebSocket handshake

WebSocket handshake 尽量复用相同身份/路由 Header：

- originator；
- x-client-request-id；
- session-id、thread-id；
- compatibility headers；
- x-codex-routing-hint；
- x-oai-attestation；
- OpenAI-Beta: responses_websockets=2026-02-06；
- 可选 x-responsesapi-include-timing-metrics；
- provider / auth headers。

连接建立后，后续 response.create message 仍携带 client_metadata。已有 turn_state 会写入 message-level client_metadata["x-codex-turn-state"]，而不是要求每次都重新建立 HTTP Header。

### 8.3 Header merge precedence

Responses WebSocket 的 merge_request_headers() 当前顺序为：

    provider headers
      -> extra headers 覆盖同名 provider headers
      -> default headers 只填补仍为空的名称
      -> auth headers 最后追加

这说明固定 provider 配置、动态 request context、默认客户端身份和 credential 并不是一个无序 HeaderMap。

---

## 9. 响应 Header 总表

### 9.1 请求关联、模型与 turn state

| Header | 消费位置 | 用途 | 行为 |
|---|---|---|---|
| x-codex-turn-state | HTTP/SSE 与 WebSocket | turn sticky routing token | 保存首值，同 turn 重放 |
| x-request-id | SSE response | upstream request correlation | 转成 ResponseStream.upstream_request_id / telemetry |
| x-oai-request-id | error debug context | OpenAI request correlation fallback | 错误诊断路径读取 |
| openai-model | HTTP / WS response | 实际 server model | 产生 ServerModel / model verification 信息 |
| x-openai-model | stream event embedded headers | server model 的兼容名称 | event parser 与 openai-model 等价识别 |
| X-Models-Etag | HTTP / WS response | models catalog etag | 产生 ModelsEtag event / catalog freshness 信息 |
| x-reasoning-included | HTTP / WS response | 标识服务端包含 reasoning | 转成 ServerReasoningIncluded |

### 9.2 Rate-limit Header 家族

Codex 支持默认和动态命名的 rate-limit family。limit id 规范化后形成：

    x-<limit>-primary-used-percent
    x-<limit>-primary-window-minutes
    x-<limit>-primary-reset-at

    x-<limit>-secondary-used-percent
    x-<limit>-secondary-window-minutes
    x-<limit>-secondary-reset-at

    x-<limit>-limit-name

未给 limit id 时默认 legacy family 为 x-codex-*。parse_all_rate_limits() 还会通过任意 x-<limit>-primary-used-percent 自动发现其他 metered limit。

额外 Codex quota / credit Header：

- x-codex-credits-has-credits
- x-codex-credits-unlimited
- x-codex-credits-balance
- x-codex-promo-message
- x-codex-rate-limit-reached-type

这些 Header 用于 usage / quota UI 和限流状态，不是 cache/session state。

### 9.3 Safety buffering Header

- x-codex-safety-buffering-enabled
- x-codex-safety-buffering-faster-model

客户端从 HTTP 或 WebSocket event headers 解析 SafetyBufferingTreatment。faster-model 可以在 enabled=false 时仍作为 fallback 信息存在，因此两个字段不是简单的同开同关布尔组。

### 9.4 错误与边缘诊断

错误响应 debug context 还读取：

- cf-ray：Cloudflare request/ray correlation；
- x-openai-authorization-error：认证失败类型；
- x-error-json：base64/JSON 形式的结构化错误信息，用于提取错误 code；
- x-request-id / x-oai-request-id：请求关联。

这些值适合诊断和 telemetry，不应作为稳定路由或缓存 key。

### 9.5 标准 HTTP 控制

Retry-After、HTTP status、Content-Type 等标准 Header 仍参与通用 backoff、错误分类和 streaming detection，但它们不属于 Codex 私有 session/metadata 体系。

---

## 10. 各 Header 的功能归类

### Cache / affinity

- body prompt_cache_key
- HTTP session-id（ChatGPT root agent 时通常映射 effective prompt cache key）

### Thread / lineage

- thread-id
- x-client-request-id
- x-codex-parent-thread-id
- x-codex-window-id

### Turn sticky routing

- x-codex-turn-state

### Backend routing / feature gating

- x-codex-routing-hint
- x-codex-beta-features
- OpenAI-Beta
- x-openai-internal-codex-responses-lite
- x-responsesapi-include-timing-metrics

### Agent / operation classification

- x-openai-subagent
- x-openai-memgen-request
- x-codex-turn-metadata

### Authentication / product identity

- Authorization
- ChatGPT-Account-ID
- X-OpenAI-Fedramp
- originator
- User-Agent
- x-openai-internal-codex-residency
- x-oai-attestation

### Response observation

- x-request-id / x-oai-request-id
- openai-model / x-openai-model
- X-Models-Etag
- x-reasoning-included
- x-<limit>-* rate-limit family
- x-codex-credits-*
- x-codex-promo-message
- x-codex-rate-limit-reached-type
- x-codex-safety-buffering-*
- cf-ray / x-openai-authorization-error / x-error-json

---

## 11. 对 OpenBridge v2 的约束

### 11.1 不建立一个万能 SessionId

OpenBridge execution context 至少应概念上区分：

    LogicalSession
    CacheAffinity
    ThreadIdentity
    ContextWindowIdentity
    TurnState
    RoutingHint
    AgentLineage
    RequestIdentity

不是所有 Provider 都需要这些字段，也不是所有字段都应由 Gateway 自动生成。

### 11.2 可以自动派生的内容

对普通 stateless Responses 客户端：

- 可以从 final IR 的稳定 prompt prefix 与 principal scope 派生 CacheAffinityKey；
- 对 ChatGPT profile，可把同一 K 映射到 body.prompt_cache_key 与 HTTP session-id；
- 可以根据最终 model/service tier 产生 provider-owned routing hint，前提是该 profile 有明确证据；
- 可以生成 OpenBridge 自己的 per-request trace identity，但不要自动冒充 Codex thread-id。

### 11.3 不能无依据自动生成的内容

没有真实 runtime owner 时，不应伪造：

- logical session identity；
- thread-id / parent-thread-id；
- context-window lineage；
- x-openai-subagent；
- x-codex-turn-metadata 的 agent topology；
- x-codex-turn-state。

尤其 x-codex-turn-state 只能由服务器签发和按 turn 重放。

### 11.4 Provider-private Header 不进入 Task IR

这些 Header 影响缓存、routing、observability、agent topology 或认证，但不改变“模型应该完成什么任务”。因此应由 RequestExecutionContext / Provider Adapter / Credential 分层拥有，而不是写进 Generation semantic IR。

### 11.5 日志与隐私

session/cache/thread/turn key 虽然通常不是 credential，也可能是隐私敏感、高基数 metadata：

- 不记录完整值到普通日志；
- 不作为 metrics label；
- OTLP 仅在明确 policy 下使用不可逆或低敏感度投影；
- evidence / fixture 不保存真实 account、session、thread、turn-state；
- credential Header 永远按敏感值处理。

---

## 12. 一手源码

固定提交：44b857c00e5803adedbc5b2e94c4a33574a157fe

- core/client.rs  
  https://github.com/openai/codex/blob/44b857c00e5803adedbc5b2e94c4a33574a157fe/codex-rs/core/src/client.rs
- core/responses_metadata.rs  
  https://github.com/openai/codex/blob/44b857c00e5803adedbc5b2e94c4a33574a157fe/codex-rs/core/src/responses_metadata.rs
- core/responses_headers.rs  
  https://github.com/openai/codex/blob/44b857c00e5803adedbc5b2e94c4a33574a157fe/codex-rs/core/src/responses_headers.rs
- codex-api/endpoint/responses.rs  
  https://github.com/openai/codex/blob/44b857c00e5803adedbc5b2e94c4a33574a157fe/codex-rs/codex-api/src/endpoint/responses.rs
- codex-api/endpoint/responses_websocket.rs  
  https://github.com/openai/codex/blob/44b857c00e5803adedbc5b2e94c4a33574a157fe/codex-rs/codex-api/src/endpoint/responses_websocket.rs
- codex-api/requests/headers.rs  
  https://github.com/openai/codex/blob/44b857c00e5803adedbc5b2e94c4a33574a157fe/codex-rs/codex-api/src/requests/headers.rs
- codex-api/sse/responses.rs  
  https://github.com/openai/codex/blob/44b857c00e5803adedbc5b2e94c4a33574a157fe/codex-rs/codex-api/src/sse/responses.rs
- codex-api/rate_limits.rs  
  https://github.com/openai/codex/blob/44b857c00e5803adedbc5b2e94c4a33574a157fe/codex-rs/codex-api/src/rate_limits.rs
- codex-api/safety_buffering.rs  
  https://github.com/openai/codex/blob/44b857c00e5803adedbc5b2e94c4a33574a157fe/codex-rs/codex-api/src/safety_buffering.rs
- login/auth/default_client.rs  
  https://github.com/openai/codex/blob/44b857c00e5803adedbc5b2e94c4a33574a157fe/codex-rs/login/src/auth/default_client.rs
- model-provider/bearer_auth_provider.rs  
  https://github.com/openai/codex/blob/44b857c00e5803adedbc5b2e94c4a33574a157fe/codex-rs/model-provider/src/bearer_auth_provider.rs
- response-debug-context/src/lib.rs  
  https://github.com/openai/codex/blob/44b857c00e5803adedbc5b2e94c4a33574a157fe/codex-rs/response-debug-context/src/lib.rs

---

## 13. 结论

Codex 当前 Header 设计的关键不是“哪些字段需要透传”，而是不同字段拥有不同 owner 和生命周期：

- cache affinity 可以由客户端/Gateway 确定；
- logical session 和 thread 需要真实 runtime identity；
- turn-state 必须由服务器签发；
- routing hint 由最终 model/tier 决定；
- agent lineage 只在 runtime 明确知道时投影；
- credential/account Header 必须与执行 metadata 隔离；
- canonical turn metadata 正在向 client_metadata 集中，直接 Header 更多承担 transport / compatibility projection。

OpenBridge v2 应吸收这些生命周期边界，而不是复制 Codex 的私有 Header 名作为内部数据模型。