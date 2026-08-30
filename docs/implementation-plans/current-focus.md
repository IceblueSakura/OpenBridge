# 当前开发焦点

## 状态

**已批准：富语义 Generation IR 设计基线；仅文档与设计，不修改运行时。**

## 1. 目标

结合当前 OpenBridge 源码、确定性测试以及已经固定的外部生态调研，形成后续额外 rewrite branch 可采用的
Generation Canonical IR 设计基线。该基线必须回答静态语义、stream event、identity/state、capability/fidelity、
Provider lowering、server-side tools 和测试迁移问题；本焦点完成前不定义生产 Rust API，也不替换现有 Bridge 或
Native Path。

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
| `src/bridge/conversion/` | Chat↔Responses request/response/stream pairwise conversion | protocol-neutral transform |
| `src/bridge/chat.rs`、`responses.rs` | terminal、identity、fragmented tool arguments 和 stream accumulation | canonical Event IR |
| `src/provider/operation.rs` | fixed Provider operation/path、wire preparation、status/SSE classification | capability planner 或 tool executor |
| `src/registry/public_model/` | immutable execution interface、capability aggregation、continuation issuer constraints | request-time semantic decoder |

直接证据包括：

- `ApiRequest` 明确保存 RoutePlan 确定协议后的 JSON bytes：`src/core/request.rs:124-188`；
- analyzer 不选 Route、不改 body：`src/pipeline/generation/analysis.rs:35-41`；
- `RequestRequirements` 与 `RequestedCapabilities` 只保存规划事实：`src/pipeline/generation/types.rs:15-26`、`74-96`；
- Native candidate 目前仍保留 normalized wire body，Bridge candidate 调用 pairwise `BridgePlan`：
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
├── origin: ToolOrigin              # Downstream | GatewayPolicy | ProviderProfile
├── executor: ToolExecutor          # Client | Gateway | Provider(origin)
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

`ToolOrigin`描述声明来自客户端、Gateway注入还是Provider profile；`ToolExecutor`只描述执行owner，不授予权限。两者必须分离：
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
└── reason: ChangeReason

exact(transform) := transform.changes.is_empty()
```

不在成功结果中再放`Unsupported`，避免“带unsupported继续encode”的无效状态。证明等价的inactive-field omission属于
`Normalized`；合成downstream envelope/ID属于`Synthesized`；Gateway执行tool属于`Emulated`；删除有意义semantic属于
`Lossy`。默认`LossPolicy::Reject`；若个人配置确实需要，Route只需一个简单`Reject | Allow`开关，不引入规则DSL。

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

## 14. 完成与授权边界

本焦点完成条件：D1-D3 形成一份内部一致、能被现有fixture反证、明确alternatives/open questions的设计基线；所有外部
事实仍链接`docs/references/`，不复制动态Provider capability表。

本焦点不授权：创建rewrite branch、定义生产Rust IR types、修改runtime、公开API、Registry schema、OpenAPI、canonical
fixtures或Provider registration。设计评审通过后，用户需另行批准rewrite branch的实现焦点。
