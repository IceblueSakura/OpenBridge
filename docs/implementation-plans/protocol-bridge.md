# Chat Completions 与 Responses：Protocol Bridge 设计

## 状态

**M5 专项设计，尚未接入运行时。** 现有 Codex、Hermes、LiteLLM 和 cc-switch 调研支持“协议转换需要显式 identity/state machine”的方向；Bridge IR 边界、continuation ledger 和目标客户端兼容范围仍需真实 corpus 验证。M1–M4 代码结构已经切换，但回归门尚未执行；在补做该验证且所选 bridge slice 的 corpus/fixture 稳定前，不进入生产请求路径。实施顺序只由[架构迁移总计划](registry-architecture-migration.md)定义。

Agent Loop 的职责边界、request/stream state owner、首版 continuation 拒绝规则和后续 ledger 门见[Agent Loop 兼容与 Bridge 状态契约](agent-loop-bridge.md)。本文保留协议转换的通用表示与算法边界。

## 结论

OpenBridge 不应让所有请求都经过统一 Bridge IR。

```text
same protocol
  → Native Path
  → minimal rewrite / preserve wire

different protocol
  → Protocol Bridge
  → parse → Bridge IR → render
```

Native Path 是兼容性基线；Bridge IR 只负责跨协议可表达的公共语义。这样避免把每个 Provider 新字段都强行塞入一个“万能协议”，也避免原生请求因 IR 不认识字段而丢失能力。

首个 bridge slice 只承诺：

- text；
- 普通 function tool schema；
- function call；
- tool result；
- usage；
- 最小 terminal outcome。

Provider-hosted tools、resource/background API、opaque reasoning、Provider-bound continuation 和未知 item 默认不等价。

## 1. 设计边界

### 1.1 Native Path

当上下游协议一致：

- 保留原始 JSON/未知合法字段；
- 只解析 route/capability 所需字段；
- 改写 `model`、URL、认证和必要 header；
- 按原协议识别 SSE event、error 和 terminal；
- 不生成 Bridge IR；
- 不生成 OpenBridge 自定义 SSE event。

典型路径：

```text
Codex Responses → OpenAI Responses
Hermes Chat → OpenAI-compatible Chat
Hermes Responses → Responses Provider
```

### 1.2 Protocol Bridge

当上下游协议不同：

```text
source wire
→ source parser/validator
→ Bridge IR + ConversionPlan
→ target renderer
→ target Provider
→ target response/stream assembler
→ Bridge IR events/result
→ source renderer
```

每个协议对有显式 converter。不要让 Provider adapter、HTTP handler 或全局 router 以零散字段 rename 实现 bridge。

## 2. 转换结果等级

每个 feature/item 必须分类：

| 等级 | 定义 | 行为 |
|---|---|---|
| `exact` | 身份、顺序、内容和状态语义均保持 | 正常转换 |
| `structure_preserving` | wire 外形不同，但客户端可观察语义保持 | 正常转换并可记录说明 |
| `approximate` | 可执行，但存在明确可观察差异 | 仅在显式允许时转换，并返回 machine-readable notice |
| `unsupported` | 无法安全表达或会破坏状态 | 上游调用前拒绝 |

`Unknown` 等同于未验证，不能自动降级为 `approximate`。

## 3. 为什么字段 rename 不够

### 3.1 输入结构不同

Chat 常以角色消息序列表达：

```text
system/user/assistant/tool messages
assistant.tool_calls[]
role=tool + tool_call_id
```

Responses 使用有序异构 items：

```text
message
function_call
function_call_output
reasoning
hosted-tool items
provider-specific items
```

一个 assistant Chat message 可以包含文本和多个并行 tool calls；Responses 则可能把它们拆为多个 output item。

### 3.2 身份不同

常见 identity：

```text
response_id
item_id
call_id
output_index
chat tool index
```

它们不是可互换字段：

- `call_id` 关联 invocation 与 tool output；
- `item_id` 标识 Responses item；
- `output_index` 表示 Responses output 的逻辑顺序；
- Chat tool index 只属于流式 `choices[].delta.tool_calls[]` 组装。

### 3.3 完成状态不同

- Chat streaming 常通过 `finish_reason` 与 `[DONE]` 收束；
- Responses 有 response/item/content/arguments 等多层事件；
- `response.output_item.done` 不是 response terminal；
- usage 可能在不同位置或独立 final chunk/event 出现。

### 3.4 continuation 不同

`previous_response_id`、opaque reasoning、encrypted content 或 Provider resource ID 可能依赖 issuing backend。将它们转成无状态 Chat 历史后，未必能再次恢复等价状态。

## 4. Bridge IR

Bridge IR 只表达首批协议真正共有的语义，并允许有类型扩展；不放任通用 `provider_data: JsonValue` 成为逃生舱。

```rust
struct BridgeRequest {
    instructions: Vec<Instruction>,
    turns: Vec<TurnItem>,
    tools: Vec<FunctionTool>,
    tool_choice: ToolChoice,
    output_contract: Option<OutputContract>,
    stream: bool,
    source_state: SourceState,
}

enum TurnItem {
    Message(MessageItem),
    FunctionCall(FunctionCallItem),
    FunctionResult(FunctionResultItem),
    ReasoningSummary(ReasoningSummaryItem),
    Extension(TypedExtension),
}

struct FunctionCallItem {
    call_id: CallId,
    source_item_id: Option<ItemId>,
    name: String,
    arguments_json: String,
    logical_index: usize,
}

struct FunctionResultItem {
    call_id: CallId,
    output: ToolOutput,
}

struct TypedExtension {
    namespace: String,
    version: String,
    payload: JsonValue,
    preservation: PreservationClass,
}
```

### 4.1 三层表示

```text
Core Semantic IR
  只放跨协议稳定语义

Typed Protocol Extensions
  明确 namespace / issuer / version

ConversionPlan
  本次哪些能力 exact / approximate / unsupported
```

renderer 不认识某个 typed extension 时必须拒绝或按明确定义忽略，不能把任意 JSON 偷渡给不兼容 Provider。

## 5. ConversionPlan 与 notice

请求进入 bridge 后先形成：

```text
ConversionPlan
  source_protocol
  target_protocol
  supported_features
  item mappings
  identity mappings
  approximation notices
  rejected features
  continuation decision
```

建议 notice：

```json
{
  "code": "reasoning_summary_approximated",
  "source_protocol": "responses",
  "target_protocol": "chat_completions",
  "action": "approximate",
  "item_id": "rs_...",
  "message": "reasoning summary was rendered as an assistant text block"
}
```

对目标客户端：

- 不在 SSE 中注入未知 OpenBridge event；
- 可在非流式安全响应 header、可选 response metadata（仅当目标协议/客户端已验证）或本地结构化日志中暴露；
- `unsupported` 必须在上游调用前返回明确错误。

## 6. Chat → Responses

### 6.1 请求

按原消息顺序处理：

- system/developer instruction → Responses instructions 或相应 message item；
- user/assistant text → message item；
- assistant `tool_calls[]` → 多个有序 `function_call` item；
- `role=tool` + `tool_call_id` → `function_call_output`；
- function tool schema → Responses function tool schema；
- 无法识别的 Chat extension → capability decision。

必须保留并行 call 顺序和 call/result 关联。若 Chat 历史缺少 assistant tool call，却出现 `role=tool`，不能凭字符串猜测 issuing call。

### 6.2 响应

Responses output 转回 Chat 时：

- text/content items → assistant content；
- 连续 function calls → 同一 assistant message 的 `tool_calls[]`；
- `call_id` → Chat `tool_calls[].id`；
- stop/terminal → `finish_reason`；
- usage → Chat usage；
- hosted-tool/reasoning/provider item → exact/approximate/unsupported 规则。

`output_item.done` 只能完成一个 item，不能结束整个 Chat stream。

## 7. Responses → Chat

### 7.1 请求

按 `input[]` 顺序处理：

1. message → Chat role message；
2. 连续 `function_call` items 暂存为一个 assistant `tool_calls` group；
3. 遇到 `function_call_output` 前 flush 对应 assistant call group；
4. 将 output 转为 `role=tool`，使用不可变 `call_id`；
5. 遇到普通 message/terminal 也先 flush pending calls；
6. 不把 Responses `item_id` 伪装成 Chat `tool_call_id`。

### 7.2 continuation

若请求只提供 `previous_response_id` + 新 tool outputs，而 Chat 上游需要完整历史，只有两种安全选择：

- 命中 issuer/target/offering-bound、未过期且无歧义的 continuation ledger，补回完整 assistant call group；
- 明确拒绝并要求客户端发送完整可转换历史。

禁止：

- 仅以全局 `call_id` 猜测历史；
- 跨 Provider/Upstream Target/Offering 查找；
- fallback 后继续使用原 continuation；
- 从日志正文隐式重建。

第一版可以完全拒绝需要 ledger 的路径，先完成无状态 bridge。

### 7.3 Chat 响应转回 Responses

- Chat text delta → Responses content delta；
- Chat tool call index → per-stream call assembly；
- late/empty id/name 在完成前暂存，完成后仍无 identity 则报错；
- arguments fragments 只拼接字符串，直到完整事件后再 parse/validate；
- finish reason → response terminal mapping；
- usage-only final chunk 不得生成空文本 item；
- `[DONE]` 只在已完成必要 terminal assembly 后结束。

## 8. Stream state machines

Chat 与 Responses 必须有独立 assembler。

### 8.1 Chat assembler

追踪：

```text
choice index
assistant text buffer/state
tool index → {id, name, arguments buffer, completion state}
finish reason
usage
terminal emitted
```

### 8.2 Responses assembler

追踪：

```text
response id
output index
item id/type/status
content index
call id
arguments buffer
usage
response terminal owner
```

### 8.3 共同不变量

- 每个 request/stream 独立 state；
- arguments 可能跨任意网络 chunk 和 SSE event；
- item done 不等于 response done；
- terminal 只发一次；
- EOF before terminal 不伪装成功；
- unknown event 的保留/忽略/拒绝由 source adapter 明确；
- client cancel 终止上游和 assembler；
- 超限 arguments/content fail closed。

## 9. Tool conversion context

每次 bridge 请求建立：

```text
ToolConversionContext
  source tool declarations
  source ↔ target name mapping
  source ↔ target call identity mapping
  schema adaptations
  conversion notices
```

它在 request renderer、response parser 和 stream assembler 间共享，但不成为全局 cache。

若需要 namespace/name 改写：

- 映射必须确定且可逆；
- 碰撞在调用前拒绝；
- schema adaptation 记录 exact/approximate；
- tool result 只按映射后的 immutable call identity 关联。

## 10. Capability 决策

bridge 不按 Provider 名称猜测能力。至少判断：

```text
source/target protocol
streaming
function tools
parallel tools
structured output
reasoning
multimodal input
hosted tools
continuation
usage streaming
```

典型规则：

| 能力 | Responses → Chat 首版 |
|---|---|
| text | structure-preserving |
| function tools | structure-preserving，需 identity fixtures |
| parallel function tools | candidate，需目标 Provider 实测 |
| structured output | candidate/approximate，取决于 target schema support |
| reasoning summary | approximate 或 unsupported |
| opaque reasoning/encrypted content | unsupported |
| hosted `web_search` | unsupported；不能伪装成 client function call |
| `previous_response_id` | unsupported，除非命中受限 ledger |
| resource/background | unsupported |

## 11. Bridge re-entry 与路由

RoutePlan 在进入 converter 前已决定 source/target protocol 和 selected Upstream Target/Offering。bridge 内不得重新调用全局 Public Model resolver。

```text
RoutePlan(mode=bridge, source=responses, target=chat, upstream_target=X, offering=chat)
```

若目标 Provider 调用失败：

- 只能在首输出前考虑剩余、同 target protocol 且 capability 等价的 candidates；
- continuation/source state 不允许跨 candidate 时直接停止；
- 不允许 Responses→Chat converter 的输出再次被选中进入 Chat→Responses converter。

## 12. 实施切片

### Slice B0：Corpus 与不变量

- 记录 Codex CLI/Hermes 的实际运行版本、平台与配置快照；
- 收集 native Chat/Responses tool-loop corpus；
- 建立 identity、ordering、terminal 和 error fixtures；
- 从 CLIProxyAPI、LiteLLM、cc-switch/Hermes issues 收集负面案例。

### Slice B1：Responses → Chat 非流式

只支持 text + function tool loop，不支持 continuation ledger。

### Slice B2：Responses → Chat 流式

实现 Responses assembler 和 Chat renderer；覆盖并行 calls、arguments delta、usage 和 terminal。

### Slice B3：Chat → Responses 非流式/流式

实现 Chat assembler、Responses renderer 和 tool identity mapping。

### Slice B4：Continuation 决策

先比较：

1. 要求完整历史；
2. 本地 issuer/target/offering-bound ledger；
3. 仅支持 native continuation。

没有充分证据前不默认实现全局 ledger。

### Slice B5：异构 Provider

通过 Anthropic Messages 或等价协议验证 Core IR、typed extension 和 stop/tool semantics。

## 13. 测试性质

除示例 fixture 外，建议加入 property/invariant tests：

- `parse_A(render_A(parse_A(x)))` 在声明的 exact subset 内稳定；
- Chat tool call/result 的 `call_id` 关联完整；
- source logical order 在 target 中可预测；
- terminal event 最多一次；
- unknown/unsupported item 不会静默消失；
- bridge notice 与实际 approximation 一致；
- arguments 分片在任意边界下得到同一完整字符串；
- continuation 不跨 issuer/target/offering/expiry；
- re-entry guard 阻止递归 bridge；
- 已输出业务事件后不 fallback/stitch。

## 14. 开放问题

- reasoning summary 在 Chat 下应拒绝、转为普通 assistant text，还是使用已验证 extension？
- structured output 的共同子集如何定义？
- 多模态 content part 的首批范围是什么？
- Codex/Hermes 是否需要 OpenBridge 在响应 body 暴露 conversion notice，还是 header/本地日志足够？
- 是否需要 continuation ledger，还是要求完整历史更符合单用户核心？
- Chat → Anthropic 与 Responses → Anthropic 是否共享同一 Core IR，还是需要协议对专用扩展？

这些问题必须由目标客户端和真实 Provider corpus 决定，而不是为了“支持更多字段”提前扩展 IR。

## 15. 证据与关联文档

- [产品范围](../functional-requirements/product-scope.md)
- [客户端兼容](client-compatibility.md)
- [Hermes Chat/Responses 分析](../references/hermes/hermes-chat-responses-analysis.md)
- [LiteLLM Chat/Responses 分析](../references/litellm/litellm-chat-responses-analysis.md)
- [cc-switch 协议与工具转换分析](../references/cc-switch/cc-switch-chat-responses-tool-conversion-analysis.md)
- [OpenAI Chat Completions 协议](../references/openai/chat-completions-protocol.md)
- [OpenAI Responses 协议](../references/openai/responses-protocol.md)
- CLIProxyAPI repository：https://github.com/router-for-me/CLIProxyAPI
- Chat → Codex tool state failure example：https://github.com/router-for-me/CLIProxyAPI/issues/2132
- Responses state affinity failure examples：https://github.com/router-for-me/CLIProxyAPI/issues/2594 和 https://github.com/router-for-me/CLIProxyAPI/issues/2596
