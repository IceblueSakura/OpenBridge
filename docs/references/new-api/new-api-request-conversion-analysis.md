# new-api 请求转换系统

## 文档元数据

| 字段 | 值 |
|---|---|
| Source snapshot | `QuantumNous/new-api` @ `2d8e50bf36e94200b809dfb39e73624ec48b1e23` |
| Last reverified | 2026-08-24，本地源码复核；focused relaykit tests 通过 |
| Scope | Chat、Responses、Claude、Gemini 的请求注册、执行、字段映射、多跳和边界 |
| Verification | `nix shell nixpkgs#go --command go test ./relayconvert/...`，在 `relaykit/` 下通过 |
| Evidence boundary | deterministic Go tests 和静态源码；不证明官方协议完整性或真实 Provider 接受/执行语义 |
| Recheck trigger | DTO、converter registry、字段映射、quality、stream state 或 Provider adaptor 变化时 |

## 1. 核心形状

new-api 没有把所有协议先压入一个万能 canonical request。其核心是一个闭合的格式转换图：

```text
客户端协议 DTO
  → 根据 Go 类型推断 source RelayFormat
  → 查找显式注册的 source → target converter
  → 执行直接转换或预先声明的多步链
  → 得到目标协议 DTO
  → Provider Adaptor 做 endpoint/credential/dialect 修正
  → disabled fields / param override
  → JSON 序列化和上游发送
```

主要格式及 DTO：

| RelayFormat | 请求 DTO |
|---|---|
| `openai` | `dto.GeneralOpenAIRequest` |
| `openai_responses` | `dto.OpenAIResponsesRequest` |
| `claude` | `dto.ClaudeRequest` |
| `gemini` | `dto.GeminiChatRequest` |

格式推断位于 `relaykit/relayconvert/request_registry.go:378-386` 和 `relaykit/relayconvert/convmeta/format.go`。未知或 typed-nil request 会直接报错，
不会自动退化成不受约束的 `map[string]any`。

## 2. 三层注册结构

### 2.1 Request registry

`relaykit/relayconvert/request_registry.go:20-53` 定义：

- `RequestConverterFunc`：一个直接请求转换函数；
- `RequestConverterSpec`：ID、From、To、Quality、直接函数或 `StepConverters`；
- `RequestStep`：一次实际 hop；
- `RequestResult`：最终 value、顶层 converter、quality 和完整 steps。

注册会校验 ID、格式、重复项、direct/steps 互斥和路径连续性：`relaykit/relayconvert/request_registry.go:85-138`。

### 2.2 Response registry

`relaykit/relayconvert/response_registry.go` 为非流式 response、stream response 和 stateful chunk converter 建立独立 registry。请求和响应不是同一个函数
反向运行，而是各自拥有方向明确的 converter。

一次真实网关 round trip 通常是：

```text
client format A --request A→B--> upstream format B
upstream format B --response B→A--> client format A
```

因此请求与响应使用方向相反的两个注册项。

### 2.3 Text converter registry

`relaykit/relayconvert/text_converter_registry.go:11-40` 用 `TextConverterSpec` 把同一逻辑 `From → To` 的 request、non-stream response、stream
response、stream state factory 和 alias 统一声明。内置规格位于 `relaykit/relayconvert/text_converter_registry.go:49-249`，初始化时再写入 request/response
registry：`relaykit/relayconvert/text_converter_registry.go:252-317`。

这层避免请求、JSON 响应和 SSE 转换各自随意命名，但不表示一份 spec 自动承担完整 round trip。

## 3. Converter 调用方式

### 3.1 按目标格式

`ConvertRequest(ctx, meta, target, request)`：

1. 推断 `from`；
2. 查找 `(from, target)` 注册路由；
3. 执行对应 direct converter 或显式 chain。

入口：`relaykit/relayconvert/request_registry.go:151-172`。Claude adaptor 用它把 OpenAI Chat 请求变成 Claude Messages：
`relay/channel/claude/adaptor.go:95-104`。

### 3.2 按 converter ID

`ConvertRequestByID` 精确选择 converter，并验证实际源格式：`relaykit/relayconvert/request_registry.go:214-228`。错误 source 会被拒绝：
`relaykit/relayconvert/request_registry_test.go:674-683`。

### 3.3 显式路径

`ConvertRequestVia` 允许调用方传入格式路径：`relaykit/relayconvert/request_registry.go:174-212`。Chat 经 Responses endpoint 发送时显式声明
`openai → openai_responses`：`relay/chat_completions_via_responses.go:96-103`。

### 3.4 多跳转换

顶层 spec 可列出 `StepConverters`。运行时展开并逐步验证：`relaykit/relayconvert/request_registry.go:230-345`。Claude → Responses 的测试路径为：

```text
Claude Messages
  → OpenAI Chat
  → OpenAI Responses
```

`relaykit/relayconvert/request_registry_test.go:632-662` 同时验证最终类型、quality、steps 和 conversion chain。

系统不做运行时最短路径或最高 quality 图搜索。多跳必须由注册表预先声明，因此新增一条边不会隐式改变既有路由。

## 4. Quality 的含义和限制

文本 converter 标记：

- `good`
- `fair`
- `discouraged`

定义：`relaykit/relayconvert/text_converter_registry.go:11-17`。当前内置分级见 `relaykit/relayconvert/text_converter_registry_test.go:24-109`：

- Chat ↔ Responses：`good`；
- Chat ↔ Claude、Chat ↔ Gemini、Responses → Claude/Gemini：`fair`；
- Claude ↔ Gemini 经 Chat：`discouraged`。

当前源码中 quality 主要是结果元数据和测试合同，没有统一 admission 或 route policy 自动拒绝 `discouraged`。调用方不能把标签本身
当作语义无损证明。

## 5. Host 与 relaykit 的边界

`relaykit` 是独立 Go module：`relaykit/go.mod:1-3`。`relaykit/relayconvert/boundary_test.go:18-81` 禁止 converter import 根模块或 Gin。

转换需要请求级信息时，只通过 `convmeta.Meta`：`relaykit/relayconvert/convmeta/meta.go:12-50`，包括：

- 原始/上游模型；
- channel ID/type；
- stream；
- reasoning effort；
- 估算 prompt tokens；
- conversion chain；
- request-scoped options；
- Claude stream conversion state。

根模块的 `RelayInfo` 实现该接口；测试和外部使用者可用 `convmeta.Values`：`relaykit/relayconvert/convmeta/meta.go:72-190`。

Provider 方言通过 `convmeta.Options` 快照注入：`relaykit/relayconvert/convmeta/options.go:3-57`，包括 Claude/Gemini thinking、Claude 默认
`max_tokens`、Gemini thought signature、安全设置、image capability 和 OpenRouter dialect。零值默认关闭 adaptation。

媒体解析则通过 host 注入 `MediaResolver`：`service/request_converter.go:14-23`。converter 可以把 URL/file source 物化为 Base64，但由 host
承担下载、缓存、清理和安全边界。

## 6. 请求生命周期中的位置

普通 OpenAI-compatible handler 的顺序大致是：

```text
DeepCopy request
  → model mapping
  → adaptor selection
  → passthrough 或 adaptor.ConvertOpenAIRequest
  → 记录 conversion chain
  → system prompt policy
  → JSON marshal
  → disabled field removal
  → param override
  → upstream request
```

证据：`relay/compatible_handler.go:42-220`。

Responses handler 类似，但调用 `ConvertOpenAIResponsesRequest`：`relay/responses_handler.go:64-119`。Chat-via-Responses 是特殊路径：
先对 Chat JSON 做 disabled fields 和 param override，再转 Responses，临时切换 relay mode/path，最后调用 Responses adaptor：
`relay/chat_completions_via_responses.go:73-140`。

因此 param override 的语义并非所有路径完全一致：有的作用于 source DTO，有的作用于 target DTO。审查某字段时必须跟随具体 handler，
不能只阅读 converter。

## 7. Chat → Responses

实现：`relaykit/relayconvert/internal/oai_chat/to_oai_responses_req.go`。

### 7.1 Messages 和 instructions

system/developer 内容被提取并以 `\n\n` 拼接成顶层 `instructions`：
`relaykit/relayconvert/internal/oai_chat/to_oai_responses_req.go:282-285`。其余 user、assistant、tool 内容进入 Responses `input` items。

这会丢失多个 system/developer 消息在原 conversation 中的精确位置，是结构压缩而非无损移动。

### 7.2 Tools

Chat function tool：

```json
{"type":"function","function":{"name":"lookup","description":"...","parameters":{}}}
```

变成 Responses：

```json
{"type":"function","name":"lookup","description":"...","parameters":{}}
```

代码：`relaykit/relayconvert/internal/oai_chat/to_oai_responses_req.go:288-313`。未知 tool 采用 best-effort：尝试保留原始 map，失败时至少保留 type。

Chat function choice 的嵌套 `function.name` 被展平为 Responses `name`：`relaykit/relayconvert/internal/oai_chat/to_oai_responses_req.go:315-351`。
`parallel_tool_calls` 保留显式值：`relaykit/relayconvert/internal/oai_chat/to_oai_responses_req.go:353-356`。

### 7.3 Tokens、structured output 和 reasoning

- `max_tokens` 与 `max_completion_tokens` 取较大者映射到 `max_output_tokens`；只有源字段存在时才设置：
  `relaykit/relayconvert/internal/oai_chat/to_oai_responses_req.go:360-368`、`411-413`；
- Chat `response_format` 转为 Responses `text.format`：`relaykit/relayconvert/internal/oai_chat/to_oai_responses_req.go:358`；
- `reasoning_effort` 转为 `reasoning.effort`，并注入 `summary: detailed`：`relaykit/relayconvert/internal/oai_chat/to_oai_responses_req.go:415-419`；
- model、stream、temperature、top_p、penalties、user、store、metadata、prompt cache 和 thinking 扩展在
  `relaykit/relayconvert/internal/oai_chat/to_oai_responses_req.go:391-420` 汇总。

`summary: detailed` 是 gateway policy 注入，不来自 source request。

## 8. Responses → Chat

实现：`relaykit/relayconvert/internal/oai_responses/to_oai_chat_req.go`。

- `instructions` 变成前置 system message；
- message item 变成 Chat string/multimodal content；
- function call 变成 assistant `tool_calls`；连续 call 合并进最后一条 assistant message：
  `relaykit/relayconvert/internal/oai_responses/to_oai_chat_req.go:321-328`；
- call ID 优先 `call_id`，否则回退 item `id`：`relaykit/relayconvert/internal/oai_responses/to_oai_chat_req.go:497-506`；
- function call output 变成 tool message，object/array output 会重新序列化为 JSON string：
  `relaykit/relayconvert/internal/oai_responses/to_oai_chat_req.go:524-536`；
- function tools 和 tool choice 恢复 Chat 的嵌套结构：`relaykit/relayconvert/internal/oai_responses/to_oai_chat_req.go:331-399`；
- `text.format` 恢复为 Chat `response_format`：`relaykit/relayconvert/internal/oai_responses/to_oai_chat_req.go:402-434`；
- image、file、video part 通过 helper 投影成 Chat content part：`relaykit/relayconvert/internal/oai_responses/to_oai_chat_req.go:436-494`。

Responses 的 stateful/hosted 能力不能由 Chat 完整表达。对 conversation、previous response、hosted prompt、context management 等字段，
转换必须审查具体 reject/drop 行为，不能仅凭最终 DTO 可生成就判定兼容。

## 9. Chat → Claude Messages

实现：`relaykit/relayconvert/internal/oai_chat/to_claude_messages_req.go`。

- system 内容移出 messages，写入 Claude `system`：`relaykit/relayconvert/internal/oai_chat/to_claude_messages_req.go:388-393`；
- text/image/document 转 Claude content blocks；媒体可经 resolver 物化；
- assistant tool call 转 `tool_use` block；ID、name 和 arguments 对应 Claude id/name/input：
  `relaykit/relayconvert/internal/oai_chat/to_claude_messages_req.go:367-380`；
- tool result 转 `tool_result`；
- thinking/model suffix 通过 request-scoped Claude options 转换；
- Claude 必需 `max_tokens`。源请求、thinking adapter 和 host default 都不能提供时明确失败：
  `relaykit/relayconvert/internal/oai_chat/to_claude_messages_req.go:394-398`。

Tool arguments 会尝试解析成 `map[string]any`；失败时记录日志并可能继续使用空 object：
`relaykit/relayconvert/internal/oai_chat/to_claude_messages_req.go:369-373`。这是兼容优先行为，不能证明非法或非 object arguments 被语义保留。

## 10. Chat → Gemini

实现：`relaykit/relayconvert/internal/oai_chat/to_gemini_chat_req.go`。

- `user → user`，`assistant → model`：`relaykit/relayconvert/internal/oai_chat/to_gemini_chat_req.go:391-394`；
- system 文本拼接到 `systemInstruction`：`relaykit/relayconvert/internal/oai_chat/to_gemini_chat_req.go:400-407`；
- text 和 media 转 Gemini parts；URL/file 可物化为 `inlineData`；不支持 MIME 直接报错：
  `relaykit/relayconvert/internal/oai_chat/to_gemini_chat_req.go:365-383`；
- tool call/result 转 `functionCall`/`functionResponse`；
- 配置允许时为 function-call part 附加 thought signature：`relaykit/relayconvert/internal/oai_chat/to_gemini_chat_req.go:387-389`；
- temperature、topP、topK、maxOutputTokens、stopSequences、candidateCount、structured output、thinking 和 safety settings
  进入 `generationConfig` 或顶层 Gemini fields。

## 11. 反向转换和 ID 风险

Gemini → Chat 位于 `relaykit/relayconvert/internal/gemini_chat/to_oai_chat_req.go`。它恢复 role、media、generation config、tools 和 system instruction，
但 Gemini function call 会按局部顺序合成 `call_1` 等 ID：`relaykit/relayconvert/internal/gemini_chat/to_oai_chat_req.go:60-76`。这不能证明多轮或并行调用的 identity 稳定。

Claude → Chat 位于 `relaykit/relayconvert/internal/claude_messages/to_oai_chat_req.go`，把 system、content blocks、tool_use/tool_result 和 token 参数投影为 Chat。
目标协议无法表达的 Claude 特性可能被压缩、文本化或丢弃。

## 12. Direct edge 与 hub conversion

OpenAI Chat 是主要 hub：Claude ↔ Gemini 等路径可经 Chat 多跳。但 Responses → Claude/Gemini 又提供 direct converter：
`relaykit/relayconvert/request_registry.go:467-485`。

原因是 Responses 比 Chat 拥有更丰富的 item/state/tool 结构；先压成 Chat 可能不可逆。Gemini 直转前还会过滤无法表达的 custom tools
及对应 output：`relaykit/relayconvert/internal/oai_responses/to_gemini_chat_req_preprocess.go:20-94`。

这说明架构不是严格 canonical hub，而是：

```text
OpenAI Chat 作为常用中间格式
+ 对语义差异较大或易丢失的路径增加 direct edge
```

## 13. Conversion chain 和可观测性

每个 hop 调用 `Meta.AppendRequestConversion`，相邻重复会被去除：`relaykit/relayconvert/convmeta/meta.go:172-180`。宿主 `RelayInfo` 还记录最终上游
request format：`relay/common/relay_info.go:167-172`、`640-680`。消费日志呈现格式序列：
`service/log_info_generate.go:209-245`。

当前链只记录 RelayFormat，不记录：

- converter ID；
- quality；
- direct 或 multi-hop；
- 被拒绝、过滤或降级的字段；
- Provider dialect adaptation。

因此它适合回答“经过了哪些协议”，不足以审计“损失了哪些语义”。

## 14. 测试资产

测试层次包括：

- registry/alias/路径连续性和错误输入；
- Chat ↔ Responses、Chat ↔ Claude、Chat ↔ Gemini 字段测试；
- Responses direct converter；
- stream terminal/state；
- golden tests 覆盖多种 from/to route；
- module dependency boundary。

2026-08-24 在 `relaykit/` 下执行：

```text
nix shell nixpkgs#go --command go test ./relayconvert/...
```

`relayconvert`、`convmeta`、`internal/oai_chat`、`internal/oai_responses` 和 `kitutil` 均通过。该结果只证明当前 deterministic tests，
不证明真实 SDK consumer、Provider wire、SSE 时序、stateful continuation 或所有字段组合。

## 15. 综合评价

优点：

- format、Provider adaptor 和 host settings 边界逐步清晰；
- converter 显式注册，错误尽量启动期暴露；
- multi-hop 不做隐式图搜索；
- request/response/stream 可共享命名和方向模型；
- relaykit 独立 module，可直接做 deterministic differential tests；
- conversion steps 和 format chain 可观测。

主要限制：

- 没有单一 typed canonical IR，hub conversion 会产生不可逆压缩；
- quality 尚未成为 admission policy；
- 部分路径 best-effort 保留、静默过滤或合成 ID；
- provider dialect、param override 和格式转换分布在不同层，必须追踪完整 handler；
- conversion log 不记录字段级 loss；
- deterministic test 通过不能替代官方 wire 和真实 Provider 语义验证。

因此该实现最适合作为一个显式转换图、字段 fixture 和第二实现参考，而不是无条件的协议 oracle。
