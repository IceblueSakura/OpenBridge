# Agent Loop 兼容与 Protocol Bridge 状态契约

## 状态

**Working hypothesis；设计已收敛到可实施边界，仍待真实 Codex/Hermes corpus 验证。**

本文细化 Chat/Responses bridge 在 Agent tool loop 中的职责、状态所有权和拒绝规则。它不改变 OpenBridge 的产品边界：Agent client 负责工具执行、审批、sandbox、取消和下一轮请求；OpenBridge 仅保留或转换 wire-level tool call/result，并在不能安全表达时拒绝。

本文的主要输入是：

- [Codex 工具生命周期调研](../references/codex-oauth-and-tool-call-analysis.md)；
- [Hermes Chat/Responses 调研](../references/hermes-chat-responses-analysis.md)；
- [cc-switch Chat/Responses tool 转换调研](../references/cc-switch-chat-responses-tool-conversion-analysis.md)；
- [Chat/Responses Protocol Bridge 设计](protocol-bridge.md)。

这些来源证明了目标客户端和参考实现的有限行为，不自动证明 OpenBridge 对真实 Provider 的兼容性。

## 1. 已收敛的决策

| ID | 决策 | 状态 | 理由 |
|---|---|---|---|
| ALC-01 | OpenBridge 不执行普通 function/custom/MCP tool；仅处理 call/result 的协议保真或转换。 | 当前边界 | 工具执行的权限、sandbox 和取消属于 Agent runtime，不能隐式进入 proxy。 |
| ALC-02 | 同协议 Agent loop 永远走 Native Path；Bridge 不解析或重建原生未知字段/event。 | 当前边界 | Native wire 保真是初期兼容性方向。 |
| ALC-03 | 初始双向 bridge 仅支持 text、普通 function schema/call/result、usage 与必要 terminal outcome。 | 当前设计选择 | 限定为可 fixture 验证的最小共同语义。 |
| ALC-04 | 初始 bridge 是**无状态的**：`previous_response_id`、opaque reasoning 或仅含 tool output 的跨轮请求一律在上游调用前拒绝。 | 当前设计选择 | 在真实 corpus 前不引入可能跨 issuer/deployment 重放的 ledger。 |
| ALC-05 | `call_id`、Responses item/response id、Responses `output_index` 与 Chat stream tool index 是不同类型，不能互相替代。 | 当前边界 | 工具 output 只能按不可变 `call_id` 关联。 |
| ALC-06 | 每个 bridge request 只有一个 stream state owner；item completion 不等于 response completion，terminal 最多一次。 | 当前边界 | 避免 Chat/Responses event lifecycle 混淆和 stream stitching。 |
| ALC-07 | provider/model 名称启发式、全局 `call_id` 猜测、从日志正文重建历史均不进入核心。 | 当前边界 | 这些做法不可审计，也违反 state affinity。 |
| ALC-08 | 受限 continuation ledger 是后续独立决策，不是首个 bridge 的前置能力。 | 后续再评估 | 只有无状态 bridge 的真实 Agent corpus 明确需要它时才实现。 |

“当前设计选择”只表示用于指导下一个测试的边界，不表示已有真实客户端兼容性证据。

## 2. 职责与状态所有权

```text
Agent client
  model response → tool invocation / approval / execution → tool output
                                               │
                                               ▼
OpenBridge
  native wire preservation
  OR bridge parse → ConversionPlan → render / assemble
                                               │
                                               ▼
Provider
  upstream protocol, provider-native state and terminal semantics
```

OpenBridge 的桥接状态严格分三层；任何状态都必须有明确 owner、生命周期和容量上限。

| 层级 | 状态 | Owner / 生命周期 | 禁止事项 |
|---|---|---|---|
| Route | `RoutePlan`、credential binding、capability decision、fallback boundary | 请求开始固定，直到 response 释放 | reload 或 fallback 后改变 issuing deployment。 |
| Request / stream | `ConversionPlan`、`ToolConversionContext`、argument buffer、item/output ordering、terminal state | 单个 request/stream；cancel、terminal 或错误即释放 | 用作跨 request cache，或由多个 renderer 并发写入。 |
| Deferred ledger | 可恢复的 assistant call group 与明确允许的 continuation reference | 仅未来显式启用；有 TTL、容量和 route binding | 以全局 `call_id`、日志或“当前唯一”作为查找键。 |

建议的实现边界：`bridge` 模块拥有 Request/stream state；`routing` 只生成不可变 RoutePlan；`provider` 只提供协议解析/渲染所需的 capability 和 adapter 行为；`ingress` 不保存 tool history。

## 3. Bridge invocation contract

每个跨协议请求在调用上游前必须构造并冻结下列上下文：

```rust
struct BridgeInvocation {
    route: RoutePlan,
    plan: ConversionPlan,
    tools: ToolConversionContext,
    source_identity: SourceIdentity,
    continuation: ContinuationDecision,
    limits: BridgeLimits,
}

enum ContinuationDecision {
    NotRequested,
    Rejected { reason: ContinuationRejectReason },
    // Future: Restored(ContinuationLease)
}
```

其中：

- `route` 固定 source/target protocol、selected deployment、credential binding、fallback boundary 和 config version；bridge 不得重新进行 alias 选择。
- `plan` 为每个请求 feature/item 记录 `exact`、`structure_preserving`、`approximate` 或 `unsupported`；只要任一必须 item 为 `unsupported`，在上游调用前失败。
- `tools` 只持有当前 request 声明和转换产生的映射，不写入全局 cache。
- `source_identity` 包含下游 protocol 的 response/item/call identity；不得从 display text 推导。
- `limits` 至少约束 request、SSE event、arguments buffer、tool count、item count 和 pending output 的字节数；超限 fail closed。

`ConversionNotice` 是 `ConversionPlan` 的审计结果，不是下游 SSE event。没有目标协议/客户端已验证的 metadata 位置时，它只能进入脱敏的本地结构化日志；不能改变 Agent 可观察的事件序列。

## 4. Identity 与工具转换契约

| Identity | 唯一职责 | 来源 | 不得替代为 |
|---|---|---|---|
| `call_id` | function/custom tool call 与 tool output 的关联键 | Responses call id 或 Chat `tool_calls[].id` | item id、response id、output index、Chat tool index |
| `item_id` | Responses item 的生命周期或 provider state | Responses item | `call_id` |
| `response_id` | response/continuation 的 issuing identity | Responses response | 任一 tool identity |
| `output_index` | Responses 输出的逻辑顺序 | Responses item/renderer | network arrival order |
| Chat tool index | 单一 Chat stream 内 fragments 的组装位置 | `delta.tool_calls[].index` | 最终 `call_id` |

`ToolConversionContext` 的最小形状：

```rust
struct ToolConversionContext {
    declared_tools: Vec<SourceTool>,
    name_mapping: BTreeMap<SourceToolName, TargetToolName>,
    call_mapping: BTreeMap<CallId, TargetCallIdentity>,
    adaptations: Vec<SchemaAdaptation>,
    notices: Vec<ConversionNotice>,
}
```

规则：

1. 普通 function tool 的 name/schema 能 exact 或 structure-preserving 映射时才可进入 bridge。
2. 连续 Responses `function_call` 必须先形成一个 Chat assistant `tool_calls[]` group，再输出其 `role=tool` result；不得按 role 预分组而改变 source order。
3. 并行 tool call 保留 source logical order。Chat 的 fragment index 只用于 assembler；最终 result 仍以 `call_id` 回接。
4. name、schema 或 call identity 映射出现冲突、缺失或不可逆时，在上游调用前拒绝。不能使用函数名或参数内容猜测 call。
5. `custom`、`namespace`、`tool_search`、provider-hosted tool 或未知 built-in item 默认 `unsupported`。未来若有 route-specific capability 和真实 corpus，须作为新的 mapping class 单独设计；不得静默降为普通 function。

## 5. 非流式与流式生命周期

### 5.1 非流式

```text
validate source wire
→ build ConversionPlan + ToolConversionContext
→ reject unsupported / continuation-requiring request
→ render target request
→ receive target response
→ parse into Bridge result
→ render source response
```

非流式 response 仍需区分 response-level outcome、tool call、usage 与 provider error；不能把含 `tool_calls` 的成功 envelope 归并成普通 text completion。

### 5.2 流式

```text
complete SSE event
→ target protocol assembler (only owner of mutable state)
→ Bridge event/result
→ source protocol renderer
→ downstream SSE bytes
```

共同行为：

- HTTP chunk 不是协议 event；SSE decoder 先提供完整 event，再交给 assembler。
- arguments 可跨任意 event 分片；只有完整 call identity 和完成条件满足时才渲染完成的 call/item。
- 空 id/name fragment 不覆盖已有 identity；late id/name 可缓冲至该 item 完成，仍缺失则失败。
- `response.output_item.done` 只能完成一个 Responses item；Chat `[DONE]` 或 Responses terminal 只能在必要的 item/usage assembly 后发出。
- `response.completed`、`response.failed`、`response.incomplete`、Chat 明确 finish reason、provider error、EOF-before-terminal 与 client cancel 是可区分 outcome；EOF 不能伪造正常成功。
- 下游已经收到业务 event 后，不得 fallback、重试拼接或使用另一 candidate；client cancel 释放 upstream stream 与所有 request state。

## 6. Continuation：首版拒绝与后续 ledger 门

首版 bridge 的 preflight 触发以下任一条件即返回明确 4xx（不调用 upstream）：

- 存在 `previous_response_id`；
- input 中出现 tool output、reasoning 或 provider state，但缺少同请求可转换的完整 assistant call/history；
- 出现 opaque/encrypted reasoning、provider resource id、background state 或未知 continuation item。

错误应说明“当前 bridge 不支持该跨轮 state”，不得将上游的缺失上下文错误伪装为 transient retry。

若真实 corpus 证明 bridge 必须补回前轮 assistant call group，才能创建独立 ledger 设计。其最小 key/binding 必须同时包含：

```text
response_id
issuer / provider family
deployment_id
route snapshot version
source and target protocol
created_at / expires_at
ordered call group and call_id set
opaque-state replay policy
```

恢复只允许命中同 issuer、同 deployment、同 protocol pair、未过期且无歧义的 entry。不能跨 candidate fallback；不能使用全局唯一 `call_id` fallback；不能从普通日志或已脱敏 fixture 恢复。若未命中，保持拒绝或要求客户端发送完整可转换历史。

启用 ledger 前必须通过：容量/TTL 淘汰、reload 行为、cancel cleanup、并发访问、issuer/deployment mismatch、expiry、工具并行 group、fallback 禁止和 secret/log isolation 测试。

## 7. 失败分类与可观察性

| 类别 | 失败 | 下游行为 | 是否可 retry/fallback |
|---|---|---|---|
| Preflight | unsupported feature、identity 缺失、continuation 要求、超限 | 上游调用前 OpenAI-style 4xx | 否 |
| Request render | 目标协议不可表达、schema mapping 冲突 | 上游调用前 4xx/5xx 配置错误 | 否 |
| Upstream before business output | connect/timeout、明确可重试 HTTP failure | 遵循 immutable RoutePlan 的首输出前策略 | 仅等价 route 且无 continuation |
| Stream assembly | invalid event、identity 永不完整、arguments 超限、terminal conflict | 关闭当前 stream，记录安全诊断 | 否 |
| Upstream terminal/error | failed/incomplete/provider error | 以源协议可表达的终态或安全错误传递 | 输出后否 |
| Client cancel | downstream disconnect/cancel | 取消 upstream，释放 context；不补发 terminal | 否 |

普通日志只记录 request id、route id、协议对、conversion classification、有限错误分类和长度/计数。不得记录 tool arguments、tool output、reasoning opaque payload、credential 或完整 request transcript；真实 wire 仅存放于已脱敏 corpus。

## 8. 双向 bridge 的 fixture matrix

| 行为 | Responses → Chat | Chat → Responses | 共同断言 |
|---|---|---|---|
| F1 text | `input[]` text/multi-turn → Chat messages | Chat roles → Responses input | 顺序与 terminal 不变；Native Path 不受影响。 |
| F2 tool request | 连续 function calls → assistant tool group + tool results | assistant `tool_calls[]` + tool results → ordered function items | `call_id` 一对一，禁止 name/id 猜测。 |
| F3 streaming | Chat argument deltas → source Responses lifecycle | Responses output/content/argument events → Chat chunks | 任意分片、late/empty identity、并行 calls、usage-only final。 |
| F4 failure | Chat error/EOF/cancel → source outcome | Responses failed/incomplete/EOF/cancel → source outcome | item done 不等于 terminal；terminal 最多一次。 |
| F5 rejection | hosted/opaque/continuation/unknown item | 同左 | 在 upstream call 前拒绝且有安全诊断。 |
| F6 real Agent | Codex → Chat-only Provider | Hermes Chat → Responses-only Provider | 记录实际版本、脱敏 wire、真实多轮 tool loop。 |

每个 fixture 至少记录：客户端与 Provider 版本、route/config snapshot、原始或脱敏 request/SSE bytes、client-observed event、预期 terminal、证明范围和未证明范围。外部项目源码案例仅用于生成 fixture，不替代 F6。

## 9. 收敛后仍待验证的假设

| 假设 | 最小验证 | 可能的修订触发 |
|---|---|---|
| 无状态双向 bridge 足以覆盖首批工作流 | Codex/Hermes 真实多轮普通 function-tool corpus | 客户端只发送 `previous_response_id + output` 且目标 Chat 需要历史。 |
| Chat tool index 可稳定恢复为 Responses output order | 并行、late id/name、乱序 fragments corpus | 上游不提供稳定 index 或多个 choices 的语义不可表达。 |
| 初始仅普通 function tool 不损害目标场景 | client/provider corpus 包含的 tool type 清单 | 目标场景依赖 custom/namespace/hosted tool。 |
| route-bound state affinity 已足够 | reload、fallback、cancel、expiration negative tests | provider 证明还需额外 issuer/account/resource binding。 |
| protocol-pair assemblers可独立实现 | 双向 stream fixture 与 memory/backpressure baseline | 需要共享状态导致 terminal/identity 分支泄漏。 |

在这些实验完成前，本文保持 Working hypothesis；任何“真实 Agent Loop 已兼容”的说法必须有对应 corpus 链接。

## 10. 关联文档

- [客户端兼容](client-compatibility.md)
- [Chat/Responses Protocol Bridge 设计](protocol-bridge.md)
- [Provider 适配与数据流](provider-adapters-and-dataflow.md)
- [原生协议验证记录](../implementation-status/native-protocol-validation.md)
