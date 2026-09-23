# 网关请求执行上下文设计：Session、Cache 与 Provider 执行元数据

## 1. 文档状态与目的

本文是一份**实现前设计基线**，用于下一阶段本地代码修订。它记录本轮已经明确的方向、当前实现起点、建议的数据流、所有权、安全边界、首个实施切片与验收条件。

本文**不是新的 ADR，也不表示功能已经实现**。现有 IR 权威、阶段顺序、来源保留、Event 生命周期继续分别由 ADR-0001～0005 维护；本文只细化“任务 IR 之外、Provider wire 之前”的网关执行上下文。实际实现如果改变跨模块职责或形成新的长期不可逆选择，再判断是否需要回写 ADR。

核心目标是：

> 客户端表达任务语义；OpenBridge 统一管理请求级执行上下文；Provider Adapter 负责把 provider-neutral 的执行提示映射为具体 header/body 字段；credential 仍由独立敏感边界管理。

这样，普通 OpenAI-compatible 客户端不需要理解 OpenRouter、Grok、Codex 或其他上游的 session/cache 私有扩展，也不需要为每个 Provider 分别注入自定义 header。

---

## 2. 问题背景

聚合网关和 Agent 型客户端会逐渐遇到一类不属于模型任务语义、但直接影响上游执行效果的信息，例如：

- 会话或工作流亲和性：session / conversation / agent-run affinity；
- prompt cache scope 或 cache routing key；
- Provider 专用 request/trace identity；
- 完整响应缓存策略；
- 固定应用 attribution、routing metadata 等。

以 OpenRouter 为例，截至 2026-09-21 的官方资料明确：

- `session_id` 可作为请求 body 顶层字段，也可通过 `x-session-id` header 提交；二者同时存在时 body 值优先；最大 256 字符；
- `session_id` 用作 sticky routing key，用于把同一 conversation/agent workflow 尽量固定到同一 Provider，并用于 observability grouping；
- 没有显式 session 时，OpenRouter 可根据 opening messages 等信息派生会话指纹；显式 session 可以让 stickiness 从首次成功请求开始生效；
- OpenRouter 的 **response caching** 与 Provider prompt caching 是两套不同机制。response cache 可通过 `X-OpenRouter-Cache`、`X-OpenRouter-Cache-TTL`、`X-OpenRouter-Cache-Clear` 控制，并可能直接返回完整历史响应而不调用模型；
- OpenRouter 的 prompt caching、sticky routing、response caching 不能合并成一个抽象的 `cache=true`。

官方参考：

- https://openrouter.ai/docs/api/api-reference/presets/create-presets-chat-completions
- https://openrouter.ai/docs/api/api-reference/presets/create-presets-responses
- https://openrouter.ai/docs/guides/routing/routers/auto-router
- https://openrouter.ai/docs/guides/best-practices/prompt-caching
- https://openrouter.ai/docs/guides/features/response-caching
- https://openrouter.ai/blog/tutorials/prompt-caching-sticky-routing/

OpenBridge 当前已经有部分相关基础，但所有权还没有统一：

- `SafeHeaders` / `SensitiveHeaders` 已经把普通上游 header 与认证 header 分离；
- `ProviderRequestHeaders` 适合编译期固定 User-Agent / attribution / Provider identity；
- `build_routed_outbound_headers` 已经具有“选定 operation/model 后再施加 trusted routed policy”的位置；
- OpenRouter 当前注册了 `prompt_cache_key: true`，但 `transform_request_headers` 仍为空；
- Generation planner 已有 candidate-specific `prompt_cache_key` 过滤；
- 当前 `prompt_cache_key` 更接近“客户端提供、Gateway 判断能否转发”，还不是 Gateway 主动生成和管理的 execution hint。

因此，下一步需要解决的不是“允许更多自定义 header”，而是建立一层明确的 **Request Execution Context**。

---

## 3. 所有权原则

### 3.1 四类信息必须分开

| 信息类别 | 例子 | Owner | 是否进入 Task IR |
|---|---|---|---|
| 模型任务语义 | messages、tools、temperature、structured output、reasoning | Task IR | 是 |
| 网关执行上下文 | session affinity、prompt cache scope、request/trace identity、response cache policy | OpenBridge Request Execution Context | 否 |
| Provider wire 表示 | `x-session-id`、`session_id`、Provider 私有 cache/header/body 字段 | Provider Adapter | 否 |
| 凭据与认证 | Authorization、API key、OAuth token | Credential / SensitiveHeaders | 否 |

关键约束：

1. Task IR 只回答“模型应该做什么”，不承担 Provider routing/cache 的执行技巧。
2. Execution Context 只保存 provider-neutral 的执行意图和 opaque key，不保存具体 Provider header 名。
3. Provider Adapter 决定最终用 header、body、query 还是不发送。
4. credential 永远不能通过 Execution Context 或 SafeHeaders 注入。
5. 原始 downstream header 不得成为绕过 OpenBridge policy 的 Provider-private 透传通道。

### 3.2 不把 session/cache 变成第二套语义 IR

Execution Context 可以影响“在哪里执行、怎样提高 cache 命中、是否允许复用完整响应”，但不能改变任务内容本身。

因此：

- 删除或修改 IR 中的正文、工具、reasoning 等语义后，Execution Context 不得恢复旧值；
- session/cache key 不参与任务等价判断；
- cache key 相同不表示两个 GenerationRequest 语义相同；
- affinity key 相同只表示“允许/希望共享执行亲和性”，不表示可以复用 response。

---

## 4. 目标数据流

建议的目标请求路径为：

```text
downstream request
  → authentication / principal resolution
  → fixed Public Model + task contract
  → task/protocol decode
  → final Request IR
  → semantic validation / trusted semantic processing
  → derive semantic requirements from final IR
  → resolve RequestExecutionContext
       ├─ request identity
       ├─ affinity context
       ├─ prompt-cache context
       └─ response-cache policy
  → fixed preflight + fixed Route plan
  → each candidate independently
       ├─ semantic lowering / representability
       ├─ Provider execution-metadata mapping
       ├─ Provider wire encode
       └─ credential binding
  → trusted transport
```

这里有三个重要顺序要求：

1. **Execution Context 不替代 IR。** prompt cache key 如果依赖 instructions/tools/schema，应从最终 IR 或其稳定投影派生，而不是从未验证 raw JSON 直接 hash。
2. **Execution Context 不改 Route 顺序。** 第一版 cache/affinity 都是 soft execution hints；某个 candidate 不支持时按既有固定计划省略，不在请求期动态重新选 Provider。
3. **Provider mapping 晚于 provider-neutral context。** OpenBridge 先得到“需要 affinity/cache hint”这一事实，再由候选 Adapter 决定具体 wire。

---

## 5. 概念数据模型

第一版不要求一次性建立通用框架，下面只定义需要保持的概念边界。

```rust
struct RequestExecutionContext {
    request: RequestIdentity,
    affinity: Option<AffinityContext>,
    prompt_cache: PromptCacheContext,
    response_cache: ResponseCachePolicy,
}

struct AffinityContext {
    source: AffinitySource,
    key: OpaqueAffinityKey,
}

enum AffinitySource {
    ExplicitGatewaySession,
    DerivedConversationFingerprint,
}

struct PromptCacheContext {
    source: PromptCacheSource,
    key: Option<OpaquePromptCacheKey>,
}

enum PromptCacheSource {
    Disabled,
    GatewayDerived,
    ClientHint,
}

enum ResponseCachePolicy {
    Disabled,
    // Future only:
    // Eligible { ttl: Duration }
}
```

上述名字只是设计说明，不要求按同名 Rust 类型实现。首个切片应优先保持最小 API，并且在第二个任务真正需要共享之前，不提前制造跨任务通用框架。

### 5.1 Request identity

Request identity 用于一次 downstream 请求及其 retry/fallback attempts 的稳定关联：

- 同一次 downstream 请求内保持不变；
- 不是 user/session identity；
- 不用于 prompt cache 内容寻址；
- 不进入模型语义；
- 可以参与低敏感度 tracing，但不能成为高基数 metrics label。

### 5.2 Affinity context

Affinity 表达：

> “这些请求如果属于同一逻辑 session/workflow，希望映射到相同的上游亲和性 key。”

它与 prompt cache key 必须分离。

一个 affinity key 可以帮助 OpenRouter sticky routing，但不能被解释为“两个请求共享相同 prompt prefix”。

### 5.3 Prompt cache context

Prompt cache 表达：

> “这些请求共享一段稳定、可缓存的 prompt 前缀或 cache namespace。”

它不能用于跳过模型执行，也不能自动开启 OpenRouter response cache。

### 5.4 Response cache policy

完整响应缓存会直接改变执行语义：cache hit 时可能不调用模型，而返回历史完整 response。

因此第一版固定：

```text
ResponseCachePolicy = Disabled
```

除非未来单独完成产品合同、数据保留、随机性、tool/side-effect、stream replay 与安全评估，否则不自动发送 `X-OpenRouter-Cache: true`。

---

## 6. Session / affinity：能自动管理什么，不能假装知道什么

### 6.1 Stateless OpenAI HTTP 的基本限制

对于普通：

```text
POST /v1/chat/completions
POST /v1/responses
```

OpenBridge 不能可靠区分：

- 同一 conversation 的下一轮；
- 用户重试；
- 两个独立但 opening messages 相同的新 conversation。

因此**不能凭空制造“绝对正确的 session identity”**。

这意味着第一版不得建立一个看似智能、实际会错误合并会话的全局 session detector。

### 6.2 允许的三种层级

#### A. 明确的 Gateway-owned session

未来如果 OpenBridge 自己拥有 logical session handle，则它是最高可信来源。

可能来源包括：

- OpenBridge-native session surface；
- Agent runtime 与 OpenBridge 的内部调用约定；
- 将来可选的单一通用 `X-OpenBridge-Session`。

即使未来暴露 `X-OpenBridge-Session`，客户端也只需要理解 OpenBridge 一次，而不是分别理解 `x-session-id`、`x-grok-conv-id`、Provider body session 等。

该通用入口**不属于首个切片的必需项**。

#### B. Derived conversation fingerprint

可以从经过验证的 canonical opening context 派生 best-effort affinity，例如：

- authenticated principal scope；
- Public Model；
- 稳定的 opening messages / instructions 指纹。

必须明确标记为 `DerivedAffinity`，不能把它描述成真实 session ID。

默认是否启用需要独立实现决策；如果无法证明误合并风险可接受，则第一版可以不发 session header，让 OpenRouter 使用自己的隐式 fingerprint。

#### C. No affinity

如果没有可信 logical scope：

```text
affinity = None
```

Provider Adapter 不发送 session header/body。

**宁可不优化，也不能为了 cache hit 伪造错误的会话归属。**

---

## 7. Prompt cache key：适合由 OpenBridge 主动管理

相比 session identity，prompt cache scope 更适合 Gateway 自动派生，因为它可以建立在 final IR 的稳定前缀上。

### 7.1 第一版建议纳入 key 的内容

候选输入应来自 canonical、稳定、对缓存前缀有实际影响的部分，例如：

- authenticated principal / tenant scope；
- Public Model；
- system/developer instructions；
- function tool declarations；
- tool JSON Schema；
- structured-output schema；
- 其他明确属于“稳定 prompt prefix”的 canonical 配置。

### 7.2 默认不纳入

以下内容不应为了简单而全部 hash 进 key：

- 最新 user turn；
- 最新 tool result；
- request ID；
- retry attempt number；
- Provider/Target identity；
- credential；
- endpoint URL；
- temperature/max output token 等不会定义 prompt prefix 的普通执行控制。

否则 key 会随每轮变化，失去 cache affinity 价值。

### 7.3 key 不是完整内容 hash

PromptCacheKey 的意义是 routing/cache namespace hint，而不是证明“两个完整请求相同”。

因此：

```text
same prompt-cache key
≠ same GenerationRequest
≠ safe response reuse
```

### 7.4 生成方式

key 必须：

- opaque；
- 有明确 domain separation/version；
- 至少按 authenticated principal scope 隔离；
- 不包含可直接恢复的用户名、prompt、project name 等原文；
- 长度满足最严格目标 Provider 的上限；
- 稳定 canonical serialization，不能依赖 JSON key order 或 Rust Debug 输出。

推荐结构概念上类似：

```text
keyed_hash(
  "openbridge:prompt-cache:v1"
  || principal_scope
  || public_model
  || canonical_stable_prefix
)
```

具体使用 HMAC-SHA256、keyed BLAKE3 或现有依赖中的同等级 keyed hash，在实施时根据现有依赖最小化决定；本文不为了设计文档强制新增 crypto dependency。

第一版可使用 process-local random epoch key：

- 进程生命周期内稳定；
- 重启后自然失效；
- 不需要新增持久 secret 配置；
- 不适合跨实例/跨重启共享 cache affinity，但这不是当前目标。

只有真正需要多实例/跨重启 affinity 时，再引入 cluster-scoped derivation secret。

---

## 8. 现有 client `prompt_cache_key` 的迁移策略

当前 OpenBridge 已经把 `prompt_cache_key` 作为部分 Public Model/Provider 的可转发字段。为了避免一次改动同时破坏兼容性，建议分阶段处理。

首个切片：

1. 如果客户端没有提交 `prompt_cache_key`，且目标 Public Model policy 允许 Gateway-managed prompt cache，则 OpenBridge 派生 key。
2. 如果客户端已经提交且当前公开合同允许，先保持既有行为，不在同一切片中改变其语义。
3. Gateway-derived key 与 client-provided key 必须有明确 precedence，不能两个都进入最终 wire。
4. 后续再决定是否把 client key 从“raw exact forward”改为“logical cache hint 后重新 opaque 化”。

建议第一版 precedence：

```text
explicit accepted client key
    > gateway-derived key
    > no key
```

这样可以先获得“普通客户端无需扩展也能使用 cache hint”的能力，而不顺带制造 breaking change。

长期如果产品决定全面 GatewayManaged，再单独收窄客户端 override。

---

## 9. Provider Adapter 映射

Execution Context 中不出现 Provider header 名。

### 9.1 OpenRouter 示例

推荐概念映射：

| provider-neutral context | OpenRouter wire |
|---|---|
| affinity key | 优先 `x-session-id` header |
| prompt-cache key | 目标 API 已确认支持时映射到对应 prompt-cache wire 字段/策略 |
| response cache disabled | 不发送 `X-OpenRouter-Cache: true` |
| request identity | 默认不暴露；只有明确 Provider contract 才映射 |
| static attribution | 继续由 Provider static header policy 管理 |
| credential | Authorization，由 SensitiveHeaders 最后追加 |

对 OpenRouter，优先使用 `x-session-id` 而不是 body `session_id` 的理由是：它本质属于 routing/execution metadata，不是模型输入语义。官方同时支持 body 和 header，若未来 OpenRouter profile 对某 endpoint 有不同约束，再由 Adapter 决定。

### 9.2 ChatGPT / Codex Responses 映射

截至 2026-09-22 对 openai/codex 提交 44b857c00e5803adedbc5b2e94c4a33574a157fe 的固定复核表明，ChatGPT Responses 的 HTTP session-id 不能简单建模为 logical session：

- Codex 先得到 effective prompt cache key：显式 override 优先；特定 internal child 场景可按 parent thread 派生；否则回退到真实 metadata.session_id。
- Responses body 的 prompt_cache_key 使用该 effective key。
- 对 root agent，ChatGPT HTTP session-id 同样使用 effective prompt cache key，Codex 源码明确说明 ChatGPT 从该 Header 派生 cache affinity。
- 对 non-root agent，session-id 保留真实 metadata.session_id；parent/thread/subagent 等 lineage 另有独立 metadata。
- thread-id 与 x-client-request-id 当前跟随真实 thread identity，不应由 cache key 代替。
- x-codex-turn-state 是服务器签发的 turn-scoped sticky-routing token，只能在同一 turn 接收并原样重放，不能由 Gateway hash/UUID 生成。

因此 Provider-neutral context 需要进一步区分：

    LogicalSession
    CacheAffinity
    ThreadIdentity
    TurnState

对没有 session 扩展的普通 Responses 客户端，OpenBridge 可以从 final IR 的稳定 prompt prefix 派生 CacheAffinityKey K；ChatGPT Adapter 在证据支持的 profile 中可以映射：

    body.prompt_cache_key = K
    HTTP session-id       = K

这不表示 OpenBridge 已经识别出真实 logical session。没有真实 runtime identity 时，不自动制造 thread-id、parent-thread-id、x-openai-subagent 或 x-codex-turn-state。

完整固定证据与 Header 生命周期见 [Codex Responses HTTP / WebSocket Header 行为基线](../references/codex/codex-responses-http-header-behavior.md)。

### 9.3 Header 组装顺序

现有结构可以演化为：

```text
base safe headers
  → legacy/sanitized downstream header policy
  → fixed Provider headers
  → trusted RequestExecutionContext mapping
  → sensitive authentication headers
```

要求：

- execution context 映射不能写 Authorization/Cookie/Host/Proxy-Authorization；
- static Provider header 与 dynamic execution header 如果声明同名且语义冲突，应 fail closed 或在编译期拒绝，而不是依赖隐式覆盖顺序；
- credential 永远最后追加；
- downstream Provider-specific header 不能覆盖 Gateway-managed key。

### 9.4 Body 映射

如果某 Provider 只能通过 JSON body 接收 session/cache metadata：

- 由 Provider Adapter 在最终 target/profile mapping 阶段注入；
- 不写回 Task IR；
- 不允许 generic request_body_hook 成为任意业务语义编辑器；
- 注入字段需要有明确 Provider contract、预算和冲突规则。

---

## 10. Raw downstream headers 的长期边界

当前 Provider hook 可以看到 `downstream_headers: &HeaderMap`。这个能力应谨慎使用。

长期方向：

```text
raw downstream headers
  → ingress-owned extraction / validation
  → trusted OpenBridge request context
  → Provider Adapter
```

而不是：

```text
client x-session-id
  → OpenRouter x-session-id
```

直接透传。

原因：

1. Provider 私有扩展会重新把客户端和具体 Provider 耦合。
2. raw header 可能包含隐私或未来敏感字段。
3. fallback 到另一 Provider 时，原 Provider header 没有语义。
4. 客户端可以绕过 Gateway 的 session/cache policy。

首个切片不必删除现有 header hook，但新的 session/cache 能力不应继续走 raw passthrough。

---

## 11. 普通但私密的执行 header

session/cache/trace key 通常不是 credential，所以不应该伪装成 `SensitiveHeaders`；但它们仍可能是隐私敏感、高基数 execution metadata。

因此实现需要明确“non-auth but private”的处理：

- 可以继续通过 `SafeHeaders` 的认证隔离规则发送；
- header value 在 HTTP/debug 层应标记为 sensitive 或通过专用插入 helper 防止值被 Debug/trace 输出；
- 本地 request-header 内容日志必须把 session/cache/trace key 加入强制脱敏；
- OTLP attribute、metrics label 不记录原值；
- 错误信息只报告 header name / policy，不回显 key；
- 不在 evidence/corpus 中保存真实 session/cache key。

不要仅因为名称不含 `token` 或 `secret` 就认为 `x-session-id` 可以明文日志。

---

## 12. Retry、fallback 与固定路由

### 12.1 同一次 downstream 请求

同一个 RequestExecutionContext 必须被所有 attempts 共享：

```text
downstream request
  → one execution context
  → candidate A attempt 1
  → candidate A retry
  → candidate B fallback
```

不能每次 attempt 重新随机生成 session/cache key。

### 12.2 不改变固定 Route plan

第一版 affinity/cache 都是 soft hint：

- candidate 支持：映射；
- candidate 不支持：省略；
- 不因“不支持 cache key”动态跳过 candidate；
- 不重新排序固定 Route；
- 不扩大 capability intersection；
- 不读取 live Provider 状态决定 key。

未来如果出现“业务必须保持 affinity，否则请求无意义”的强约束，应成为新的显式产品语义，而不是把 soft cache hint 偷偷升级为路由硬条件。

### 12.3 跨 Provider fallback

内部可以保持同一个 opaque affinity/cache scope，但每个 Provider 独立映射：

```text
OpenBridge affinity key
  ├─ OpenRouter → x-session-id
  ├─ Provider B → provider-specific body field
  └─ Provider C → omitted
```

不得把 `x-session-id` 本身当成跨 Provider 的 canonical 字段。

---

## 13. Response cache：第一版明确不做

OpenRouter response caching 会在命中时跳过真实模型调用，并返回已缓存的完整响应。官方还说明：

- 默认关闭；
- 可设置 TTL；
- streaming/non-streaming 均可缓存；
- tool-call response 也可缓存；
- cache hit 的 usage 可能为零；
- stochastic 参数不会阻止返回相同缓存结果。

这与 prompt cache/sticky routing 的风险完全不同。

因此第一版：

- 不自动发送 `X-OpenRouter-Cache: true`；
- 不增加 TTL 配置；
- 不增加 cache clear API；
- 不把 response cache status 当作模型 usage 的普通等价结果；
- 不为 retry 自动开启 response cache。

未来若启用，需要单独评估：

- temperature/randomness；
- time-sensitive prompts；
- tool/side-effect workflows；
- data retention / ZDR；
- streaming replay；
- usage/accounting；
- cache hit observation；
- clear/TTL precedence。

---

## 14. 第一实施切片建议

为了符合“宁缺毋滥”，建议下一次本地代码修订只做一个最小闭环，不同时适配多个 Provider。

### 14.1 范围

**目标：建立最小 RequestExecutionContext，并让 OpenRouter 能消费 Gateway-managed prompt-cache hint；session affinity 只建立类型/映射边界，不伪造不存在的 logical session。**

具体：

1. 在 Generation request planning 附近增加最小的 execution-context value/resolver；暂不抽象为全任务公共框架。
2. 从 final Generation IR 的稳定 prefix 派生 gateway prompt-cache key。
3. 客户端未提供 `prompt_cache_key` 时才注入 gateway-derived key，保留现有客户端兼容路径。
4. 每个 candidate 独立决定是否映射/省略 key，不改变 Route。
5. OpenRouter Adapter 接收 execution context，而不是从 raw downstream header 取 session/cache provider field。
6. 为 affinity 预留 provider-neutral value，但没有可信 session source 时保持 `None`。
7. 不启用 OpenRouter response cache。
8. 不引入 session database、跨进程 store、plugin/hook 框架。

### 14.2 暂不做

- 通用 `X-OpenBridge-Session`；
- 自动全局 conversation registry；
- 基于 TCP connection 的 session；
- 从单个 prompt 猜测真实 session；
- response cache；
- 多 Provider 一次适配；
- 热更新 Provider metadata；
- cluster key / Redis；
- Provider-specific client header passthrough。

### 14.3 物理代码位置原则

不要预先为了“未来所有任务”创建大而空的 `execution_context` framework。

首个切片优先放在最接近 Generation planning 的 owning layer。只有第二个任务（如 Embeddings 或 Images）确实需要共享 request execution metadata 时，再提取共享基础值。

`src/execution.rs` 当前拥有 attempt/candidate runtime state，不应因为名字相似就顺手变成 session/cache policy owner。

---

## 15. 最小测试与验收

只增加能够保护独立机制的测试。

### 15.1 Pure derivation

至少覆盖：

1. 同 principal + Public Model + instructions/tools/schema → key 稳定；
2. instructions 变化 → key 变化；
3. tool schema 变化 → key 变化；
4. structured-output schema 变化 → key 变化；
5. 只有最后 user turn / tool result 改变时，若它不属于稳定 prefix，key 不应被无意义打散；
6. principal 不同 → key 不同；
7. key 不包含原始 prompt/tool/schema 明文；
8. canonical JSON key order 不影响结果。

### 15.2 Provider mapping

OpenRouter 最低 owning layer：

- gateway prompt key 被映射到预期 wire；
- client 已有合法 key 时遵循明确 precedence；
- unsupported candidate 省略而不是重新选路；
- raw client `x-session-id` 不自动成为 upstream affinity；
- dynamic execution header 不能覆盖 auth；
- Provider static header 与 execution header 冲突 fail closed；
- 日志/debug 不出现 key 原值。

### 15.3 Router smoke

只有当最低层测试不能证明“同一 context 在 retry/fallback attempts 间保持稳定”时，才增加**一个** production Router smoke。

不要为每个 Provider/model/route 建矩阵。

### 15.4 不需要 corpus case 的内容

key derivation、SafeHeaders 注入、Provider mapping 属于确定性运行时机制，优先 Rust owner tests；没有新的 wire 协议事实时，不为增加数量而制造 `testdata/cases`。

---

## 16. 与现有 IR / cache 状态的关系

当前 Generation `RequestState.cache()`、`prompt_cache_key` capability、candidate filter 仍是迁移起点。

实施时需要避免两套 owner：

- Task IR / RequestState 可以继续表达当前客户端显式 cache hint；
- RequestExecutionContext 表达 Gateway 自己计算出的 execution cache scope；
- 最终 Provider wire 必须有单一 precedence；
- source body 不能在 Gateway key 删除/替换后恢复旧 `prompt_cache_key`；
- requirements 只表达任务/公开接口要求，不因为 Gateway 可优化 cache 就把 cache hint升级成模型能力硬要求。

如果后续决定 GatewayManaged 完全替代 client forwarding，应作为独立兼容性变更处理，不与首个切片混做。

---

## 17. 设计结论

本轮建议固定以下原则：

1. **Execution Context 是 Task IR 与 Provider wire 之间的独立网关层。**
2. **session affinity、prompt cache、response cache 是三种不同语义。**
3. **OpenBridge 主动管理 provider-neutral key；Provider Adapter 管理 header/body 映射。**
4. **客户端不需要理解每个 Provider 的 session/cache 扩展。**
5. **没有可信 logical session 时宁可不发送 session key，也不伪造会话身份。**
6. **prompt cache key 可以优先由 final IR 的稳定前缀自动派生。**
7. **现有 client `prompt_cache_key` 首个切片保持兼容，Gateway-derived key 只补空缺。**
8. **response cache 第一版默认关闭。**
9. **key 必须 opaque、principal-scoped、不可从日志/metrics 泄漏。**
10. **execution hints 不改变固定 Route 顺序、retry/fallback 边界或 credential 所有权。**
11. **首个实现只做 OpenRouter + Generation 的最小闭环，不提前建通用 plugin/session store。**

这套边界允许 OpenBridge 后续逐步吸收 OpenRouter、Grok、Codex 等 Provider 的执行元数据差异，同时保持“客户端任务语义 → Task IR → 固定路由 → Provider mapping”的主数据流清晰。
