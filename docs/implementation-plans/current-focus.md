# 当前开发焦点

## 状态

**R0-R6已完成；R7 Native takeover与旧路径删除已获准实施。**

## 1. 目标

在`feature/generation-ir-rewrite`继续实现R7：让Native路径也执行canonical decode/project/check/lower，满足条件时使用typed
`PreserveSource` patch，否则重新encode，并删除旧Native request与Responses buffering语义路径。R7不新增ACL/策略DSL/动态plugin，
不改变下游HTTP API、配置schema、Registry schema、OpenAPI或canonical fixture，并保持precommit retry/fallback、postcommit禁止
fallback、cancel、body/event bound与terminal fail-closed边界。

最终设计对象是一次模型交互的内部语义，不是 OpenAI Chat、Responses 或任一 Provider DTO 的重命名版本。

## 2. 使用背景与复杂度预算

OpenBridge 主要是个人使用的可信配置网关，不建设多租户 model/provider 授权系统。设计优先：

1. immutable data + pure transformation；
2. closed enum/newtype 表达 portable semantic；
3. 显式 `Result`、fidelity 和 lowering disposition；
4. 小 facade、按 semantic domain 拆分的 leaf modules；
5. 由可信 Bootstrap/Registry 决定 Target、URL、credential 和 Provider profile。

仍需保留的安全边界只有现有架构已经依赖的技术正确性：业务请求不能选择任意 URL、credential、auth header 或
transform script；Provider-private state 不能错误 replay 到不兼容 Target；bounded body/event/resource limit、
commit 后禁止 fallback、credential secrecy 和 fail-closed unknown semantic 不因个人使用而取消。

明确不引入：

- per-user/per-model ACL；
- 通用规则 DSL、动态策略图或 plugin policy engine；
- 为每个语义对象建立 trait hierarchy；
- 通过 Provider name 分散 `match` 的“简化”；
- portable semantic 中的任意 `serde_json::Value`；
- 长期 Native/IR 双栈、compatibility shim 或 legacy alias。

## 3. 当前事实基线

当前实现已经包含多个彼此独立但不完整的隐式语义层：

| 当前 owner | 已拥有的事实 | 不是 |
|---|---|---|
| `src/core/request.rs` | `OperationKind`、Chat/Responses `ApiProtocol` 和 protocol-bound JSON bytes | semantic IR |
| `src/pipeline/generation/analysis.rs` | registry-independent request facts、unknown/reserved field rejection | Route selector 或完整 decoder |
| `src/pipeline/generation/types.rs` | capability requirements、ordered candidates、fallback/state-affinity flag | request/response content model |
| `src/pipeline/generation/preflight.rs` | Public Model fixed contract 与 value-sensitive capability validation | encoder |
| `src/pipeline/generation/planning.rs` | normalization、candidate lowering、Native/Bridge materialization | canonical semantic owner |
| `src/bridge/static_codec/`、`event_codec/` | production Chat↔Responses lowering与per-request Event state | Route selector或transport owner |
| `src/bridge/responses.rs` | R7前Native Responses buffering所需的terminal assembly | production Protocol Bridge state |
| `src/provider/operation.rs` | fixed Provider operation/path、wire preparation、status/SSE classification | capability planner 或 tool executor |
| `src/registry/public_model/` | immutable execution interface、capability aggregation、continuation issuer constraints | request-time semantic decoder |

直接证据包括：

- `ApiRequest` 明确保存 RoutePlan 确定协议后的 JSON bytes：`src/core/request.rs:124-188`；
- analyzer 不选 Route、不改 body：`src/pipeline/generation/analysis.rs:35-41`；
- `RequestRequirements` 与 `RequestedCapabilities` 只保存规划事实：`src/pipeline/generation/types.rs:15-26`、`74-96`；
- Native candidate 目前仍保留normalized wire body，Bridge candidate调用IR-backed `BridgePlan`：
  `src/pipeline/generation/planning.rs:24-34`、`83-145`；
- `previous_response_id` 使 cross-target fallback 关闭：`src/pipeline/generation/planning.rs:150-155`；
- Provider adapter 已按 operation 固定 protocol/path，并拒绝不匹配 request：`src/provider/operation.rs:132-182`。

## 4. Canonical semantic inventory

设计必须逐项给出 canonical owner、wire decoder、capability requirement、lowering 和测试不变量：

| Domain | 必须表达的语义 |
|---|---|
| Instruction/conversation | instruction source、ordered user/assistant items、role 与 content ordering |
| Content/resource | text、image、audio、file；URL/inline/provider reference 仅是 source variant |
| Tool declaration | function、server-side、Provider-native；schema/config 与 execution owner 分离 |
| Tool lifecycle | declaration、call、arguments、result、error、approval、source/citation 和 identity |
| Reasoning | requested effort、visible text、summary、opaque replay state 与 visibility |
| Output constraint | unconstrained text、JSON object、JSON Schema 与 strictness |
| Generation control | output limit、sampling/stop/seed、parallel-tool policy；保留 absent/value distinction |
| State | continuation reference、Provider session/cache 和 opaque state 分离 |
| Response | ordered output items、finish reason、status、usage、Provider metadata |
| Streaming | lifecycle event、delta identity、terminal、error、usage、EOF 与 materialization |

`RequestRequirements` 中已有的 capability facts 应由 canonical request 纯投影产生，而不是继续与 full request 平行解析；
media size/source aggregate、requested parameter set 和 stream usage request仍是 planning projection，不应塞回每个 IR node。

## 5. IR 边界

### 进入 IR

- downstream 请求表达的 portable model-interaction semantic；
- decoder 能可靠识别的 Provider-native declaration 或 opaque state；
- upstream response 解码后的 ordered semantic items/events；
- identity、visibility、execution owner、state affinity 和 fidelity 所需标记。

### 不进入 IR

- Public Model alias、Route order、Target URL、credential、auth/proxy headers；
- retry/cooldown/health、transport client、HTTP status classification；
- request/response body limit 的执行器状态；
- tracing span、JSONL record 或 Provider raw transcript；
- Agent run/step/subagent、tool implementation 和 arbitrary orchestration state；
- Embeddings、Images Generations 等独立 operation 的现有 DTO。

IR 可以投影 capability 和 observability，但不得反向依赖 Registry、RoutePlan、transport 或 telemetry。

## 6. 设计不变量

1. Chat、Responses 以及后续 Messages/Gemini wire 只存在于 decoder/encoder 边界。
2. 同协议不代表同 capability；Native fast path 只能是可证明保持 IR semantic 的优化。
3. item 与 content 顺序默认具有语义，不在 decoder 中合并 reasoning/text/tool items。
4. Gateway identity 与 Provider wire identity 分离；synthetic ID 不伪装成 Provider replay ID。
5. opaque state 带 namespace 与 affinity；不理解 payload 也不能 arbitrary passthrough。
6. unknown portable semantic fail closed；Provider extension 只有目标 profile 显式接受时才能 encode。
7. capability check 在 lowering 前完成，encoder 只编码已决定的 lowering。
8. loss、normalization、synthesis、omission 和 emulation 可机器观察，不由 adapter 静默执行。
9. Event IR 必须能 materialize 为与 non-stream IR 等价的 terminal response。
10. commit 前可 retry/fallback，commit 后任何第二 Provider 输出都不得拼接。

## 7. 本设计焦点的三个 checkpoint

### D1：边界与库存

本节即 D1：固定现有 owner、semantic inventory、复杂度预算和非目标。

### D2：Static IR 与 lowering

形成最小富语义 algebra、identity/state、Provider extension、capability projection、fidelity report 和纯函数签名；用现有
Chat/Responses fixtures做 paper walkthrough。

### D3：Event IR、server-side tools 与迁移验证

形成 event algebra、materializer、server-tool 注入/剥离/执行边界、IR-native test layers 和 rewrite branch 原子替换门槛。

## 8. D2 推荐的 Static IR algebra

以下名称是语义草图，不是最终 Rust API。推荐顶层保持少量 owned value：

```text
GenerationRequest
├── input: Vec<InputItem>
├── tools: Vec<ToolDefinition>
├── tool_choice: ToolChoice
├── output: OutputConstraint
├── controls: GenerationControls
├── state: RequestState
└── extensions: Vec<ProviderExtension>

GenerationResponse
├── id: ResponseId
├── candidates: Vec<Candidate>
├── status: ResponseStatus
├── usage: Usage
└── extensions: Vec<ProviderExtension>

Candidate
├── id: CandidateId
├── output: Vec<OutputItem>
└── finish: Option<FinishReason>
```

`candidates` 不能假定只有一个：Chat `n`、Gemini candidates 等富语义需要自然表达；Target capability 或 downstream
protocol不支持多个 candidate 时，由 lowering拒绝，而不是decoder丢弃。

### 8.1 Ordered items

```text
InputItem
├── Instruction(Instruction)
├── Message(InputMessage)
├── PriorToolCall(ToolCall)
├── ToolResult(ToolResult)
├── ReasoningReplay(ReasoningItem)
└── Extension(ProviderExtension)

OutputItem
├── Message(OutputMessage)
├── Reasoning(ReasoningItem)
├── ToolCall(ToolCall)
├── ToolResult(ToolResult)       # Provider/Gateway executed tool
└── Extension(ProviderExtension)
```

Instruction成为ordered item并保留authority/source；Message只是item的一种。Chat system/developer message与Responses
top-level instructions可归一为同一语义，但该归一必须进入fidelity report。Tool call、tool result和reasoning不嵌入一个
assistant message，避免丢失item identity与lifecycle。

Input/Output enum分开，防止把downstream-only instruction或未完成stream fragment构造成合法response；底层共享
`Instruction`、`MessageContent`、`ToolCall` 等value object，避免重复逻辑。

### 8.2 Content 与 resource

```text
MessageContent
├── Text { text, annotations }
├── Image(ImageResource)
├── Audio(AudioResource)
└── File(FileResource)

ResourceSource
├── Url(UrlValue)
├── Inline(BoundedBytes)
└── ProviderReference(OpaqueState)
```

media kind、media type、detail和source分别建模。URL、inline和Provider file ID不是三种semantic media，而是同一resource
的source variant。Inline bytes、URL长度、item/count预算仍由ingress/requirements projection执行；IR value本身不拥有全局
allocator或I/O。

Text annotation至少预留source/citation引用；citation指向独立`SourceId`/`SourceRef`，不把Provider raw annotation塞进text。

### 8.3 Tool semantic

```text
ToolDefinition
├── name: ToolName
├── origin: ToolOrigin              # Downstream | GatewayPolicy(ToolPlanId) | UpstreamProvider(ProviderOrigin)
├── executor: ToolExecutor          # Client | Gateway | Provider(origin)
├── visibility: ToolVisibility      # Public | Internal
└── kind: ToolKind

ToolKind
├── Function { description, input_schema, strict }
├── Server(ServerToolConfig)        # WebSearch / FileSearch / Code / ...
└── Extension(ProviderExtension)

ToolCall
├── id: CallId                      # canonical correlation identity
├── tool: ToolName
├── input: ToolInput                # closed JSON object or typed server input
└── provider_identity: Option<WireIdentity>

ToolResult
├── call_id: CallId
├── status: ToolResultStatus        # Success | Error | Denied
├── output: Vec<ToolOutput>         # Text | Json | Resource | Source
└── provider_identity: Option<WireIdentity>
```

Function arguments在completed Static IR中必须是bounded JSON object；malformed/raw fragments只存在于Event IR reducer，结束时
解析失败。这样避免同时维护raw string与parsed JSON两个事实源。JSON Schema使用validated/bounded `JsonSchema` newtype；
它可以内部包装JSON object，但不能退化成通用portable payload。

`ToolOrigin`描述声明来自客户端、Gateway注入还是upstream response；`ToolExecutor`只描述执行owner，不授予权限。两者必须分离：
Gateway可以注入一个由Provider执行的web search，也可以把Provider-native declaration改写为Gateway执行的function loop。
个人网关不需要ACL；可信Route/Target profile决定Provider/Gateway是否能执行。

### 8.4 Reasoning、identity 与 state

```text
ReasoningPart
├── VisibleText(Text)
├── Summary(Text)
└── Opaque(OpaqueState)

OpaqueState
├── namespace: ProviderNamespace
├── kind: OpaqueKind
├── payload: BoundedOpaque
├── origin: Option<ProviderOrigin>
└── exposure: OpaqueExposure       # Returnable | InternalOnly

WireIdentity
├── namespace: ProviderNamespace
├── value: BoundedString
└── origin: ProviderOrigin
```

`ItemId`、`CandidateId`、`CallId`是Gateway canonical identity；`WireIdentity`只保留Provider关联。decoder可从合法wire ID派生
canonical ID，缺失时合成canonical ID，但不得合成可replay的Provider identity。

Request state至少分开：

- `ContinuationRef`：opaque response/item continuation；
- `CacheDirective`：best-effort cache key/retention，不等于continuation；
- Provider session或thought signature：作为`OpaqueState`附着在正确item，而不是全局bag。

downstream continuation可能没有origin；planning仍使用当前Public Model的unique issuer contract。由upstream response产生并由Gateway
保存/回放的opaque state必须带origin，lowering只接受相同origin或profile明确定义的portable转换。

### 8.5 Controls、constraint、status 与 usage

`GenerationControls`保留字段absent与显式value的区别，至少覆盖output limit、temperature、top-p/top-k、stop、seed、penalties
和parallel-tool control。无效值在decoder拒绝；Target range/subset在capability check拒绝；证明等价的默认/空值移除记为
normalization。

`OutputConstraint`为`Text | JsonObject | JsonSchema { schema, strict }`。schema normalization若只改变key order可记
normalized；删keyword或降strict属于lossy/unsupported。

`ResponseStatus`与`FinishReason`分开：completed/incomplete/failed/cancelled是response lifecycle；stop/length/tool-call/
content-filter是candidate finish。HTTP/transport error仍在IR外，但HTTP 200内的failed/incomplete response属于IR。

Usage每个计数使用`Option<TokenCount>`保留missing与zero；input、cached input、output、reasoning、total分别记录。跨attempt
聚合属于observability，不写入winning response usage。

## 9. Provider extension 的简化模型

```text
ProviderExtension
├── namespace: ProviderNamespace
├── kind: ExtensionKind
├── payload: BoundedOpaqueJson | BoundedBytes
└── origin: Option<ProviderOrigin>
```

不建设schema registry服务或动态plugin系统。实现时用集中、穷举的decoder/encoder match接受已知namespace/kind；unknown
extension默认拒绝。只有Provider-private semantic允许opaque payload，portable text/tool/media/reasoning不得借extension逃避建模。

同namespace且origin兼容不自动表示可转发；Target encoding profile还必须显式声明接受该kind。下游能够发送的extension也由固定
protocol decoder决定，不能提供任意JSON passthrough入口。

## 10. Capability、fidelity 与 lowering

推荐成功变换只返回value和实际changes；unsupported直接是error：

```text
Transform<T> { value: T, changes: Vec<SemanticChange> }

SemanticChange
├── path: SemanticPath
├── kind: Normalized | Synthesized | OpaquePreserved | Emulated | Lossy
├── reason: ChangeReason
└── authorization: None | ToolDirective { plan: ToolPlanId, directive: ToolDirectiveId }

exact(transform) := transform.changes.is_empty()
```

不在成功结果中再放`Unsupported`，避免“带unsupported继续encode”的无效状态。证明等价的inactive-field omission属于
`Normalized`；合成downstream envelope/ID属于`Synthesized`；Gateway执行tool属于`Emulated`；删除有意义semantic属于
`Lossy`。默认`LossPolicy::Reject`；若个人配置确实需要，Route只需一个简单`Reject | Allow`开关，不引入规则DSL。

loss检查只有两条规则：带有效`ToolDirective` authorization的Lossy change仅授权该plan/directive产生的变化；其余Lossy change
仍受全局`LossPolicy`控制。authorization由`apply_tool_plan`的私有constructor产生，decoder/Provider adapter不能伪造。

推荐数据流和纯函数边界：

```text
decode_request(protocol, bytes, limits)
  -> Decoded<GenerationRequest>

project_requirements(&GenerationRequest)
  -> RequestRequirements

check_capabilities(&RequestRequirements, &TargetCapabilities)
  -> Result<(), CapabilityError>

lower_request(&GenerationRequest, &TargetProfile, LossPolicy)
  -> Result<Transform<TargetRequest>, LoweringError>

encode_request(TargetRequest)
  -> Result<Bytes, EncodeError>
```

response方向使用对称的`decode_response`、`lower_response`、`encode_response`。这些函数不访问Registry singleton、不选择Route、
不执行I/O，也不写telemetry；调用者显式传入immutable profile并观察report。

`TargetRequest`/`TargetResponse`是codec模块私有的typed wire value，不进入pipeline公共API。Provider quirk集中在
`TargetProfile`和对应encoder leaf，不泄漏到IR或pipeline。

## 11. Native preservation 与 source envelope

所有请求都必须decode为IR并执行同一requirements/capability检查，但不强制每次重序列化。`Decoded<T>`可在IR外携带：

```text
SourceEnvelope { protocol, original_body, observed_shape }
```

当且仅当：

1. source/target protocol及encoding profile相同；
2. capability check通过；
3. lowering不含`Emulated`或`Lossy`，且所有`Normalized`/`Synthesized`变化都能由已批准的typed wire patch实现；
4. opaque payload无需重解释或跨origin；

lowering可返回`PreserveSource { patches }`，继续满足当前Native wire-preservation合同。否则返回typed `Encode`路径。fast path是
IR validation后的优化，不能绕过decoder或capability projection。

不把original bytes、field order或whitespace写入Canonical IR；它们属于wire envelope。若后续决定不再承诺Native JSON形状保留，
可以删除fast path而不改变IR algebra。

## 12. 推荐模块 ownership

```text
src/ir/generation/          canonical values、validation、projection、fidelity
src/bridge/                 Chat/Responses/后续协议的decode/encode facade
src/pipeline/generation/    capability check、Target lowering、RoutePlan编排
src/provider/ + providers/  trusted Target encoding profile、wire quirks、HTTP binding
src/transport/              bytes/SSE framing 与body lifecycle
```

`src/ir/generation/mod.rs`只re-export稳定algebra；item/tool/reasoning/resource/state/fidelity分leaf。不要为每个Provider建trait object；
protocol与Provider family使用closed enum + pure match，并只在其owner root聚合。

## 13. D2 alternatives 与待验证项

拒绝的替代方案：

- Chat message作为唯一顶层：无法自然保存reasoning/server tool/item lifecycle；
- 直接复制Responses item：会固化OpenAI state/status/ID conventions；
- 一个Input/Output共用的巨型item enum：允许instruction出现在response等无效状态；
- trait-based visitor/codec graph：增加动态分派和状态，不优于穷举enum；
- always re-encode Native：无必要破坏当前wire-preservation合同；
- raw+parsed function arguments双事实源：容易在stream/round-trip中漂移。

D3 paper walkthrough必须验证：首个迁移切片是立即启用multiple-candidate decode/lowering，还是类型先支持而现有入口继续拒绝；
ProviderReference resource如何约束origin；Chat多message tool history映射为ordered InputItem是否无歧义；
`reasoning.encrypted_content`的Returnable条件；Response include应映射为输出投影请求还是wire hint。

## 14. D3 Canonical Event IR

Event IR描述一次Provider turn的canonical lifecycle，不承载HTTP/SSE framing、retry、downstream commit或tool executor。
同一Static IR leaf types由non-stream decoder和Event materializer共享。

```text
EventEnvelope
├── sequence: Sequence
└── event: GenerationEvent

GenerationEvent
├── ResponseStarted { response: ResponseIdentity }
├── CandidateStarted { candidate: CandidateIdentity }
├── ItemStarted { candidate: CandidateRef, item: ItemIdentity, header: ItemHeader }
├── PartStarted { item: ItemRef, part: PartIdentity, kind: PartKind }
├── PartDelta { part: PartRef, delta: PartDelta }
├── PartFinished { part: PartRef }
├── ItemFinished { item: ItemRef }
├── CandidateFinished { candidate: CandidateRef, finish }
├── UsageSnapshot { usage }
└── Terminal { terminal }

PartDelta
├── Text(BoundedTextDelta)
├── ReasoningText(BoundedTextDelta)
├── ReasoningSummary(BoundedTextDelta)
├── ToolArguments(BoundedJsonFragment)
├── Audio(BoundedAudioDelta)
└── Opaque(BoundedBytes)
```

`ResponseId`、`CandidateId`、`ItemId`、`PartId`、`CallId`、`OutputIndex`和`Sequence`使用不同newtype；index/order不充当
identity。source wire没有ID时允许生成deterministic、turn-scoped ID，但synthetic ID不进入Provider replay identity。

Event中的`*Identity`/`*Ref`是lifecycle key，不是D2的完整`Candidate`、`OutputItem`或`ContentPart`。Identity在start时固定
canonical ID、wire/provider identity和index；Ref只携带canonical ID。header只保存开始后不可变化的字段：

```text
ItemHeader
├── Message { role }
├── Reasoning { provider_identity }
├── ToolCall { call, tool, origin, executor, visibility }
├── ToolResult { call, status, origin, visibility }
└── Extension { namespace, kind, origin }

PartKind
├── Text
├── ReasoningText | ReasoningSummary
├── ToolArguments | ToolOutput(ToolOutputKind)
├── Resource(ResourceHeader)
├── Source(SourceHeader)
└── Opaque(OpaqueHeader)
```

ToolResult的status由ItemHeader固定，Text/Json/Content output与source/citation由其parts构造。reducer以identity key维护open
builders；只有part/item/candidate全部结束后，materializer才构造完整Static IR值。

一个`EventState`只对应一个Provider turn，最小状态为`response: Option<ResponseIdentity>`、`next_sequence`、按canonical ID索引的
candidate/item/part builders、latest usage、terminal和`eof_state`。Sequence从decoder起点严格单调；duplicate/out-of-order
sequence拒绝。
任一Ref必须指向已start且尚未finish的对象，child结束后才能结束parent；同一index映射到不同identity也拒绝。

每个delta先检查单event bound，reducer再检查part/turn累计bound。首个terminal前EOF记录`EofWithoutTerminal`并返回
`EofBeforeTerminal`；terminal后首个EOF记录clean end；duplicate EOF返回`DuplicateEof`，任何EOF后event返回`InputAfterEof`。

完整resource URL/reference可以在`PartStarted` header中给出并立即`PartFinished`；增量binary使用bounded delta。tool name、
call identity和execution/visibility在`ItemStarted`固定，后续delta不能漂移。function arguments只在part完成时解析成Static IR
的JSON object。

不单独定义`Error` event：Provider wire中合法、可解码的failed/incomplete/cancelled/error terminal归一为带可选failure detail的
`Terminal`。本地malformed JSON/SSE、unknown identity、argument parse、bound、invalid lifecycle和materialization失败分别作为
`DecodeError`、`ReduceError`、`MaterializeError`返回；它们不是canonical event，也不合成`Terminal(Error)`。HTTP status和body
transport error仍由Provider/transport boundary拥有。

## 15. Reducer、materializer 与 encoder

```text
reduce(state: EventState, input: EventInput) -> Result<EventState, ReduceError>
materialize(state: &EventState) -> Result<GenerationResponse, MaterializeError>
encode_event(state: EncoderState, event: &GenerationEvent)
    -> Result<(EncoderState, Vec<WireFrame>), EncodeError>

EventInput = Event(EventEnvelope) | Eof
```

wire codec的对应边界是`decode_wire(...) -> Result<Vec<EventEnvelope>, DecodeError>`；streaming decoder可逐frame返回零到多个
EventEnvelope，但一次错误立即结束该attempt。`EventInput`不携带error，因为error是`decode`/`reduce`函数的失败结果，不是模型
交互语义。

这些API是referentially transparent的value transition；实现可在函数内部mutate owned `BTreeMap`以避免复制，但不隐藏I/O、
task、clock或global cache。wire decoder先产生Event，reducer验证，target encoder可为internal/invisible event返回零个frame。

reducer不调用encoder，materializer不解析raw JSON，encoder不决定capability/fallback。non-stream response decoder可以直接构造
Static IR，但必须与等价Event replay的materialized结果做一致性测试。

## 16. Lifecycle、terminal、EOF 与 commit

必须区分：turn lifecycle、transport EOF、downstream commit和包含server-tool loop的logical operation completion。

```text
TurnTerminal
├── status: Completed | Failed | Incomplete | Cancelled | Error
├── finish: Stop | Length | ToolCalls | ContentFilter | Extension
└── failure: Option<FailureDetail>
```

不变量：

1. 每个成功通过decode/reduce的canonical turn恰好一个terminal；terminal后event一律拒绝。
2. `Eof`不是terminal；terminal前EOF返回`EofBeforeTerminal`，terminal后EOF成功。
3. Completed terminal要求candidate/item/part全部关闭且tool arguments已验证。
4. failed/incomplete/cancelled/error保持区分，可以保留partial state但不能materialize为成功response。
5. Chat `[DONE]`只是wire terminator；只有合法finish state后才能解码成Completed terminal。
6. Responses `response.completed`携带tool calls表示turn完成且finish为ToolCalls，不代表logical operation已经结束。
7. usage snapshot单调且最多产生一个最终client-visible usage；不得从text length或event count估算。

decode/reduce/materialize失败是上述terminal不变量的显式例外：失败state不能materialize，reducer不制造canonical error terminal。
执行层在downstream commit前将其作为candidate failure进入现有retry/fallback；commit后转为body error并停止。若目标wire协议要求
error frame，由downstream error mapper直接编码该局部错误；该wire frame不回流成Canonical Event IR。

downstream `CommitState`仍由`src/ingress/streaming/precommit.rs`拥有。commit点是第一个完整、Provider-valid且经downstream encoder
生成的visible frame，不是第一个IR event。commit前允许现有bounded retry/fallback；commit后禁止retry/fallback、不得制造terminal，
body error和cancellation沿现有ingress/transport路径传播。

## 17. Server-side tool transform 与 execution loop

D2的`ToolDefinition`同时表达origin、executor和visibility；D3只增加一个可信、编译后的小型计划，不引入规则DSL：

```text
ToolPlan
├── id: ToolPlanId
├── directives: Vec<ToolDirective>
├── max_turns
├── max_tool_calls
├── max_tool_result_bytes
└── deadline

ToolDirective
├── id: ToolDirectiveId
└── action: Inject(ToolDefinition) | Strip(ToolSelector)

ToolSelector { name: ToolName, origin: GatewayPolicy(ToolPlanId) }

apply_tool_plan(request: GenerationRequest, plan: &ToolPlan)
    -> Result<Transform<GenerationRequest>, ToolPlanError>
```

规则：

1. plan来自可信、immutable Registry compilation，不从downstream arbitrary JSON选择Provider、URL、credential或implementation。
   Target profile不能自行向base IR注入semantic tool；它只把共同的generic server tool lowering成Provider-native wire。
2. plan在logical operation开始时对base Request IR应用一次；retry/fallback candidate各自从同一transform结果纯lowering，不能重复注入。
3. Inject要求tool name唯一、origin等于`GatewayPolicy(plan.id)`并记录带同一plan/directive authorization的`Synthesized`；
   Strip selector必须携带同一plan.id并按
   name+origin精确命中，只允许GatewayPolicy tool并记录`Lossy`；
   命中downstream client tool返回错误，不得由encoder静默删除。
   Strip change携带`ToolDirective { plan, directive }` authorization，只授权该directive命中的路径，不会把Route全局
   `LossPolicy`切成`Allow`。
4. `visibility=Internal`允许downstream encoder隐藏Gateway/Provider server-tool lifecycle，但citation/source等public result仍保留；
   visibility filtering不是删除Canonical IR。
5. Provider executor由Target profile lowering成native server tool；不支持时返回Unsupported，除非plan显式选择Gateway executor。
6. Gateway executor把canonical ToolCall交给受信local executor，再追加同一CallId的ToolResult并发起下一turn；execution不进入reducer。
7. 首版只考虑read-only web search一类工具；mutating/approval-sensitive tool不在首个实现范围。

Gateway execution发生后，logical operation固定当前candidate/credential；Provider opaque state或Provider-executed tool一旦被观察也绑定
origin。这样无需多租户ACL，仍避免把tool result或opaque state交给另一个Provider继续生成。

server-tool loop使用独立budget，不复用candidate retry count：turn count、tool-call count、result bytes、operation deadline和总upstream
attempt上限。cancellation必须贯穿upstream、backoff和tool future；不允许detached task。第一次Gateway tool execution前不得向downstream
commit internal turn；commit后不得隐藏tool call再拼接后续assistant answer。

## 18. Usage、attempt 与 observability

Event IR只接收normalized `UsageSnapshot`；decoder负责把Provider delta累计成snapshot。字段继续使用Option保留missing/zero差异，
requested terminal usage不完整或负数时fail closed，不从payload长度估算。

区分三个口径：

- `TurnUsage`：一个成功Provider turn的usage；
- `OperationUsage`：Gateway server-tool loop中所有构成最终结果的成功turn之和，作为client-visible usage；
- `AttemptUsage`：失败retry/fallback等实际Provider消耗，只进入attempt observability，不混入client-visible response。

server-tool调用次数、search context和Provider已报告的tool usage可以进入typed usage details；价格、billing和成本计算仍在IR外。
observability从Request/Response/Event IR纯投影稳定属性，再与attempt/Route/latency等执行事实组合，不反向解析Provider raw JSON。

## 19. IR-native test layers 与首批 RED

测试按owner分层，不建立完整Provider/model inventory：

1. algebra validation：constructor、identity、ordering、bounded value和invalid state；
2. wire decoder：Chat/Responses JSON/SSE → Static/Event IR；
3. requirements projection：IR → `RequestRequirements`与现有preflight parity；
4. tool-plan transform与capability/lowering report；
5. private target DTO encoder与same-protocol semantic round-trip；
6. Event reducer/materializer与non-stream equivalence；
7. ingress integration：precommit/retry/fallback/cancel/resource lifetime；
8. 最多一个production Router smoke用于验证层间wiring。

首批RED tests：

- Chat和Responses的等价text/instruction/function request decode为相同Request IR；
- ordered reasoning→text→parallel tool calls保持独立item与identity；
- fragmented arguments按CallId独立累积，完成时只解析一次；empty/incomplete/malformed JSON拒绝；
- duplicate item/call/terminal、event-after-terminal和EOF-before-terminal返回typed error且不制造canonical terminal；
- Event materialization等于对应non-stream Response IR；
- visible reasoning、summary和opaque replay不互相污染；origin不匹配的opaque state拒绝；
- structured output三种mode保持strict/name/schema和absent/value distinction；
- server web-search Inject/Strip分别产生带plan/directive provenance的`Synthesized`/`Lossy` report，retry不重复注入；
- forged/mismatched PlanId、DirectiveId或未授权Lossy change在`LossPolicy::Reject`下拒绝；
- internal tool event可产生零downstream frame，但public source/citation仍输出；
- precommit failure可fallback，postcommit及tool/origin binding后不可cross-candidate fallback。

直接复用现有证据场景，不复制重复断言：

- `tests/bridge_conversion_contract.rs`：双向static/stream、usage、reasoning、tools、structured output；
- `tests/protocol_bridge_replay.rs`：terminal和duplicate identity；
- `tests/forwarding_contract/resilience.rs`：precommit、fallback、state affinity和cancellation；
- `tests/process_replay_contract.rs`：post-output failure、cancel和EOF；
- `testdata/cases/bridge/chat_to_responses/chat_to_responses.parallel_tools.fragmented_arguments/`；
- `testdata/cases/bridge/responses_to_chat/responses_to_chat.incomplete_arguments.stream/`；
- `testdata/cases/bridge/responses_to_chat/responses_to_chat.unsupported_hosted_tool.reject/`；
- `testdata/cases/faults/responses_native.terminal_violation/`和`responses_native.eof_before_terminal/`。

## 20. D3 paper walkthrough 决定

1. Canonical Response支持multiple candidates；首个迁移切片继续保持当前入口的single-candidate拒绝，直到candidate encode/stream tests齐全。
2. `ProviderReference` resource和`OpaqueState`只允许同origin profile encode；跨origin返回Unsupported，不尝试下载再上传的隐式迁移。
3. Chat多message/tool history按wire顺序展开为InputItem；tool call/result通过CallId关联，不按role重新分组。
4. opaque reasoning只有source decoder标记Returnable且downstream profile能exact承载同一extension时才公开；否则ReplayOnly。
5. Responses `include`是请求输出投影的semantic requirement，进入capability projection；不是可随意丢弃的wire hint。
6. Provider extension使用compile-time namespace/kind match，不建立runtime schema registry。
7. server-tool stripping仅来自显式trusted ToolPlan；默认Unsupported，不使用Provider adapter的silent omission。

## 21. Rewrite branch 实施阶段与 gate

设计通过后，在额外branch进行大范围重写；以下是branch内部checkpoint，不表示main长期双栈：

### R0：characterization

增加纯IR RED tests并冻结当前fixtures，不接入production。Gate：所有当前semantic invariant都有IR表达，未知项明确拒绝。

### R1：Static kernel

实现`src/ir/generation/` values、validation、requirements projection和fidelity；Chat/Responses request decoder在test-only路径运行。
Gate：projection与现有analyzer/preflight parity，纯tests不依赖Axum/network。

### R2：Static codecs

实现request/response lowering与private target DTO encoders；旧Bridge与新路径只在tests dual-run。
Gate：现有non-stream fixtures semantic parity；exact case再要求byte parity。

### R3：Event kernel

实现wire-event decoder、reducer、materializer和target event encoder。
Gate：stream fixtures、terminal/identity/usage/EOF tests通过，materialized结果与non-stream IR相等。

### R4：Bridge takeover

production Bridge原子切换到IR路径，保留transport-owned precommit/liveness/cancel。Gate：完整resilience和process replay通过；随后删除
pairwise converters和旧mutable Bridge state，不保留production feature flag。

### R5：Server-tool native policy

实现ToolPlan的Inject/Strip和Provider-native lowering，先不执行Gateway loop。Gate：fallback candidates语义等价、visibility和fidelity可观察。

### R6：Gateway web-search loop

只实现bounded read-only web search executor；buffer internal turns，aggregate successful turn usage，传播cancel并固定candidate origin。
Gate：turn/tool/result/deadline/attempt上限、无detached work、commit前后行为和错误语义均有测试。

### R7：Native takeover 与删除

Native路径也执行decode/project/check/lower；满足条件时使用`PreserveSource` typed patch fast path，否则encode。Gate：Native wire合同、Provider
request、完整Rust baseline通过，最终只保留一个canonical production path。

branch完成后一次性评审/合入；每个checkpoint独立commit并通过相称验证，但不要求中间commit可部署到main。

## 22. 完成与授权边界

本焦点完成条件：R0 tests先以缺失Static kernel真实失败；R1实现使其通过，并完成requirements projection parity、focused tests、
`cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings`与`git diff --check`。

R0/R1明确不实现：Chat/Responses wire decoder/encoder、Event IR、Bridge/Native takeover、server-tool执行、observability日志、历史配置
兼容层或任何R2-R7 production wiring。内部未发布API可直接采用最佳结构，不保留legacy alias或compatibility shim。R1通过后必须
停在branch gate，由用户另行批准进入R2。

R0/R1实际证据：

- RED：`cargo check --locked --test generation_ir_contract`因缺少`openbridge::ir`真实失败；
- focused：`cargo test --locked --test generation_ir_contract`、test-only Chat/Responses analyzer parity、fidelity、extension与requirements unit tests通过；
- baseline：`cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings`、
  `cargo clippy --locked --all-targets -- -D warnings`和`git diff --check`通过；
- static diff scan通过；未运行live Provider、外部SDK/Agent runtime、load或long-run验证。

R2实际证据：

- RED：canonical non-stream dual-run最初因缺少`StaticBridgePlan`真实编译失败；opaque reasoning保留与citation fail-closed测试分别先因Static decoder丢失对应语义而失败；
- focused：`generation_ir_static_codec_contract`、`generation_ir_contract`和完整`bridge_conversion_contract`通过；canonical exact cases的旧/新request与response bytes相等；
- baseline：`cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked --all-targets -- -D warnings`和`git diff --check`通过；static diff scan通过；
- R2只增加bounded pure Static codecs、closed private target DTO与test dual-run；production Native/Bridge、stream Event IR、Router、Provider、transport和observability路径未接入。

R3实际证据：

- RED：canonical event contract最初因缺少Event algebra/reducer/materializer真实编译失败；wire dual-run最初因缺少`StaticEventBridge`真实编译失败；
- focused：`generation_ir_event_contract`、`generation_ir_event_wire_contract`和完整`bridge_conversion_contract`通过；canonical stream fixtures与旧Bridge保持exact wire parity，等价stream/non-stream输入materialize为相同`GenerationResponse`；
- lifecycle/identity/resource：sequence、candidate/item/part hierarchy、fragmented tool arguments、usage monotonicity、terminal/EOF、opaque reasoning、event/part/turn/encoded-output bounds、child index、late child event与terminal snapshot rewrite均fail closed；
- baseline：`cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked --all-targets -- -D warnings`和`git diff --check`通过；static diff scan通过；
- R3只增加pure canonical Event IR与test-only wire codec dual-run；production Bridge/Native、Router、Provider、transport和observability路径仍未接入。

R4实际证据：

- production `BridgePlan`已由bounded Static request/response lowering与per-request Event renderer唯一实现；Registry request/JSON/SSE limits显式进入plan，旧pairwise converter与Chat mutable stream state已删除；
- focused：完整`bridge_conversion_contract`、Generation IR contracts与55个`forwarding_contract` tests通过；ChatGPT真实wire profile、stream usage、instruction preservation、precommit retry/fallback、postcommit禁止fallback、cancel与process replay保持通过；
- sparse Responses terminal、omitted in-progress item status与`output_item.done`才交付的opaque continuation已按实际Provider lifecycle建模，同时非空terminal snapshot rewrite、invalid usage、late child event与resource amplification继续fail closed；
- baseline：`cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked --all-targets -- -D warnings`和`git diff --check`通过；Native request path、Native Responses buffering、Provider adapter、配置和observability未接管。

R5实际证据：

- RED：`generation_ir_tool_plan_contract`最初因缺少`apply_tool_plan`/`ToolPlan`真实编译失败；specific-choice替换测试先因strip后保留`ToolChoice::Required`触发`InvalidRequest`而失败；
- focused：`generation_ir_tool_plan_contract`通过；Trusted `ToolPlan`支持Inject/Strip并记录`ToolDirective`授权的fidelity changes，`lower_provider_server_tool`仅当executor origin与`ProviderToolProfile`一致且profile声明支持该server-tool kind时成功；同名strip+inject在编译期拒绝，被strip的`Specific` choice在无剩余工具时降级为`None`；
- baseline：`cargo fmt -- --check`、`cargo test --locked`（31个suite）、`cargo clippy --locked --all-targets -- -D warnings`和`git diff --check`通过；static diff scan通过；
- R5未接入production planner、未执行Gateway web-search loop、未改变Native/Bridge路由行为；Provider tool lowering仅通过`StaticBridgePlan::prepare_with_tool_plan`暴露，当前所有provider `web_search` capability均为false。

R6实际证据：

- bounded Gateway web-search kernel固定一个candidate `ProviderOrigin`、reserved tool name和exact `ToolPlanId`；仅接受
  `Completed`单candidate、finish/tool lifecycle一致且恰好一个typed `ServerToolKind::WebSearch` call，function同名冒充、
  downstream-origin声明、并行额外call、origin drift与call/result identity异常均fail closed；
- internal continuation通过`GenerationRequest::with_appended_input`保留完整request controls，并按顺序追加`PriorToolCall`与相关
  `ToolResult`；失败physical attempt不计turn/usage，retry不切换origin，缺失usage字段保持缺失且聚合溢出失败；
- turn/tool/result/attempt continuation预算均在search前预留；预取消、await内取消、turn/search absolute deadline、32 KiB cumulative
  output bound、result amplification和usage overflow均由deterministic fake seam测试；
- baseline：`cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked --all-targets -- -D warnings`和`git diff --check`
  通过；static diff scan通过；
- R6 kernel在R7 production接入前保持`#[cfg(test)]`，未执行真实搜索、未改变Router/Provider/transport/observability或downstream commit。
