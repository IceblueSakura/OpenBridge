# cc-switch：Chat/Responses 与 Agent Tool 转换分析

## 结论

cc-switch 的 Codex 路径证明：面向 agent 的 `Responses -> Chat -> Responses` bridge 必须同时处理**请求上下文、跨请求 tool-call 关联和流式事件生命周期**，不能只做字段替换。

它对 OpenBridge C3/C4 Protocol Bridge 最有价值的参考不是其 JSON 代码本身，而是以下四个可验证的实现要点：

1. 在开始转换前，从原始 Responses request 提取一个**每请求的 tool context**，用于请求降级和响应/流式反向恢复。
2. 将 Responses 的连续 function/custom/tool-search calls 组装为一个 Chat assistant `tool_calls` message；tool result 使用不变的 `call_id` 回接。
3. 对 Chat SSE 维护每个 text、reasoning 和 tool call 的独立状态，累积 fragmented arguments，并按 `output_index` 重新发出完整 Responses lifecycle。
4. 当 Chat 上游要求“assistant tool call 紧邻 tool result”而 Codex 只发送 `previous_response_id + function_call_output` 时，保存前轮 call item 并在下一轮补回。

OpenBridge 应吸收这些状态机和 fixture 覆盖面，但仍应遵循既定的 **Bridge IR + `CapabilityProfile` + issuer-bound continuation ledger + `ConversionNotice`** 设计。cc-switch 的实现服务于本地桌面代理和 Codex 兼容性，包含 provider/model 名称启发式、行为性配置与未加 issuer/route binding 的 history fallback；这些不适合作为 OpenBridge 的通用核心契约。

## 1. 调研范围与证据边界

| 项目 | 值 |
|---|---|
| 调研仓库 | `F:/codespace/cc-switch` |
| 源码快照 | `08710d51fc04843ce217c58749677d84cf62740b` |
| 快照日期 | `2026-07-21` |
| 调研路径 | `src-tauri/src/proxy/providers/`、`src-tauri/src/proxy/{forwarder,handlers}.rs` |
| 重点路径 | Codex client 的 Responses API 接入，向 Chat Completions 或 Anthropic Messages 上游转换 |
| 非结论 | cc-switch 不是 OpenBridge 依赖，也不构成 OpenBridge 的公开协议或安全承诺 |

本文主要分析 Responses <-> Chat 路径：

- 协议/上游选择：`proxy/providers/codex.rs` 的 `should_convert_codex_responses_to_chat`（:76）和 `should_convert_codex_responses_to_anthropic`（:198）。
- Responses request -> Chat request：`proxy/providers/transform_codex_chat.rs` 的 `responses_to_chat_completions_with_reasoning`（:260）。
- Chat response/SSE -> Responses：`proxy/handlers.rs` 的 `handle_codex_chat_to_responses_transform`（:987）、`proxy/providers/streaming_codex_chat.rs` 的 `create_responses_sse_stream_from_chat_with_context`（:727）。
- 跨请求 tool-call 恢复：`proxy/providers/codex_chat_history.rs` 的 `CodexChatHistoryStore`（:43）。

cc-switch 还实现了 Anthropic Messages <-> Responses bridge，例如 `proxy/providers/transform_responses.rs` 与 `transform_codex_anthropic.rs`。它可作为 reasoning 与多模态 item 的补充案例，但不是本文对 OpenBridge Chat/Responses bridge 的主要移植目标。

## 2. 已验证的 cc-switch 数据流

```text
Codex client /v1/responses
  -> 根据 provider 明确 wire API 决定是否桥接
  -> build CodexToolContext（原始 tools + tool_search output）
  -> CodexChatHistoryStore.enrich_request（必要时补回前轮 call item）
  -> Responses request -> Chat request
  -> Chat upstream
  -> Chat final response 或 Chat SSE -> Responses response / typed Responses SSE
  -> 记录可供下一轮补全的 Responses function/custom/tool-search call
```

实际调用顺序由源码固定：

- handler 在原始 request 上调用 `build_codex_tool_context_from_request`，见 `proxy/handlers.rs:808`、`:923`。
- forwarder 在 Responses -> Chat 前调用 `CodexChatHistoryStore::enrich_request`，解析 provider 的 reasoning 配置，再调用 `responses_to_chat_completions_with_reasoning`，见 `proxy/forwarder.rs:1415-1444`。
- 成功的 Chat 流由 `create_responses_sse_stream_from_chat_with_context` 转为 Responses SSE，随后由 `record_responses_sse_stream` 旁路记录 tool calls，见 `proxy/handlers.rs:1004-1008`。
- 非流式 Chat response 经 `chat_completion_to_response_with_context` 转换并记录，入口见 `proxy/handlers.rs:1102-1137`。

这说明 tool context 必须跨越 request renderer、response renderer 和 stream renderer；它不是 provider HTTP adapter 内一个孤立的 request-only helper。

## 3. 请求侧：Responses item log 到 Chat messages

### 3.1 有序 input 与相邻 tool 语义

`append_responses_input_as_chat_messages`（`transform_codex_chat.rs:542`）遍历 Responses `input`，保留处理顺序，并维护：

- `pending_tool_calls`：连续 `function_call`、`custom_tool_call`、`tool_search_call`；
- `pending_reasoning`：待附着到后续 assistant/tool-call 的 reasoning 文本；
- `last_assistant_index`：防止 reasoning 穿过 user turn 泄漏到下一 assistant turn。

`function_call_output` 到达前会 flush pending calls，形成一个 assistant message，其后再发 Chat `role=tool` message（`transform_codex_chat.rs:602-765`）。这符合多数 Chat 上游要求的相邻结构：assistant `tool_calls[]` 必须在其 tool result 之前。

**对 OpenBridge 的含义**：Bridge IR 的有序 `InputItem[]` 不能按 role 预分组。Chat renderer 应将相邻的可合并 function calls 聚为一个 assistant message，但 `call_id` 必须保持为每个调用的不可变关联键。

### 3.2 Tool context 是可逆映射的必要条件

`CodexToolContext`（`transform_codex_chat.rs:61-234`）在 request 时建立 Chat name 与原始 Responses tool 的映射：

| Responses tool | Chat 降级方式 | 反向恢复依据 |
|---|---|---|
| `function` | Chat `type=function`，保留 parameters/description | 原 tool name |
| `namespace` 内 function | namespace/name 扁平为一个 Chat function name | `namespace_name_to_chat_name` |
| `custom` | 包装成参数仅含 `input: string` 的 Chat function | `CodexToolKind::Custom` |
| `tool_search` | 包装为 `tool_search` Chat function | `CodexToolKind::ToolSearch` |

对应实现包括 `build_codex_tool_context_from_request`（:236）、`responses_function_tool_to_chat_tool`（:1162）以及 response item 恢复函数 `response_tool_call_item_from_chat_name`（:1574）。流式 renderer 接收同一 context，才能把扁平 namespace/custom/tool-search 的 Chat tool call 恢复为适合 Codex 的 Responses item。

**对 OpenBridge 的含义**：`ToolDefinition` 之外需要每请求 `ToolConversionContext`，但其应是 Bridge IR renderer 的内部产物，而非将 provider 逻辑写入 route 配置。它至少要保存：

```text
source_tool_id / source kind
<-> target tool name and target schema
namespace/custom metadata
call-id policy
loss / emulation notices
```

未知 Responses built-in tool 不能沿用 cc-switch 的默认忽略行为（`add_response_tool` 的未知类型分支，`transform_codex_chat.rs:216-232`）。OpenBridge 必须由 capability gate 显式 `mapped`、`emulated`、`rejected` 或 `dropped`，并产生 `ConversionNotice`。

### 3.3 Schema 与 provider 特例应隔离

cc-switch 会填充空/缺失 tool `parameters` 为 object schema，并通过 provider/model 名称推断 reasoning 请求字段和返回字段；证据包括 `transform_codex_chat.rs:349-490` 的 reasoning mapping 及该文件 :2053 起的 schema fixture。

这些做法解决了现实兼容性问题，但不适合成为 OpenBridge 通用规则：

- `codex.rs:336-495` 按 base URL、provider name 与 model name 推断 reasoning 参数；同名模型在不同 hosted gateway 的行为可能不同。
- Chat renderer 会为特定上游重排/合并 system messages（`transform_codex_chat.rs:493-523`）。

OpenBridge 应将已证实的上游差异编译进对应 `ProviderAdapter`/`CapabilityProfile`，保持 Bridge IR 和通用 Chat/Responses renderer 不读取 provider 名称字符串。当前 `ProviderDescriptor` 与闭合 `ProviderKind` 的边界已在 `src/provider/mod.rs:30-124` 建立。

## 4. 跨请求 continuation：cc-switch 解决的问题与不可照搬部分

### 4.1 解决的问题

Codex 有时只在下一轮发送：

```json
{
  "previous_response_id": "resp_1",
  "input": [{"type": "function_call_output", "call_id": "call_1", "output": "..."}]
}
```

部分 Chat 上游不能从该 item 推回前轮 assistant tool call。`CodexChatHistoryStore` 因此记录 Responses output 中的 `function_call`、`custom_tool_call` 和 `tool_search_call`，并在 `enrich_request` 中按 `previous_response_id` 补回对应 call item（`codex_chat_history.rs:31-41`、`:48-194`）。它也处理并行 calls，测试断言先恢复整个 call group、再追加 tool results（:729-778）。

这个模式与 OpenBridge 已有的安全约束一致：`previous_response_id` 不能只被当成普通字段；跨协议转换可能需要 proxy 管理的 continuation state。

### 4.2 不可直接采用的缓存键与回退策略

cc-switch 的 history store 是进程内、最多 512 个 response 的 `HashMap<response_id, CachedResponse>`，并额外以全局 `call_id` 做“唯一时才使用”的 fallback（`codex_chat_history.rs:10-23`、`:261-307`）。该结构没有 issuer、provider、deployment、route snapshot、principal 或 TTL 字段。

即使唯一 `call_id` fallback 通过了 cc-switch 的局部测试（:537-644），OpenBridge 也**不能**把它作为通用跨 route 恢复策略。OpenBridge 已明确规定不跨 provider 重放 `previous_response_id` 或 opaque state（`src/pipeline/mod.rs:35-44`、`:170-174`；[C2 Provider 聚合](../../phases/02-provider-aggregation.md)）。

OpenBridge 的 continuation ledger 至少应以以下信息绑定：

```text
response_id, issuer/provider, deployment_id, route_snapshot_version,
protocol, created/expires_at,
ordered call items, opaque replay policy
```

恢复优先级应为：同 issuer + 同 deployment 的 `previous_response_id` 命中；无命中时拒绝或带 `ConversionNotice(previous_response_id_not_supported)` 继续已完整提供的 input。不能以“当前缓存中 call_id 恰好唯一”为理由跨 deployment 猜测。

## 5. Chat SSE 到 Responses SSE：值得采用的状态机形状

`ChatToResponsesState`（`streaming_codex_chat.rs:66-104`）维护以下独立状态：

```text
response_started / completed
response_id, model, created_at
next_output_index
text item state
reasoning item state + inline <think> parser
chat tool index -> {call_id, name, arguments, item_id, output_index, done}
completed output items
latest usage, finish_reason
tool context
```

关键行为均有源码和测试证据：

- 首次 Chat chunk 发出 `response.created`、`response.in_progress`（:267-279）。
- tool call 用 Chat `index` 聚合 fragmented id/name/arguments，但 `call_id` 是最终关联身份；只有 id 与 name 到齐才按连续 index 释放 `response.output_item.added`（:335-455）。
- finalization 将完整 arguments canonicalize 后发 `response.function_call_arguments.done`、`response.output_item.done`，最终再发 `response.completed`（:486-649）。
- 并行调用在较早 call 的 name 晚到时仍按 index 有序输出，见测试 `preserves_parallel_tool_order_when_earlier_name_arrives_late`（:981-1012）。
- identity fragments 中空 id/name 不会覆盖已有值，见 `preserves_tool_identity_across_empty_continuation_deltas`（:948-979）。
- 上游异常发 `response.failed` 而不再发 completed，见 :1191-1202。

OpenBridge 的 [Chat/Responses 转换设计](../../design/chat-responses-conversion.md) 已规定 `output_item.done` 不是终态、必须只由协议 terminal event 或 final aggregate 决定。这一点比 cc-switch 的 EOF recovery 更严格：cc-switch 对“已有实质输出但无 finish_reason”的 Chat EOF 合成 `status=incomplete`（`streaming_codex_chat.rs:804-819`）。OpenBridge 应保留现有 `terminal_missing` 诊断策略，不能把这种恢复路径伪装成原生正常完成。

## 6. Opaque reasoning 的安全边界

cc-switch 在 Anthropic <-> Responses bridge 中把完整 Responses reasoning item JSON 做 URL-safe Base64 编码，放入 Anthropic `thinking.signature` 或 `redacted_thinking.data`，再尝试恢复（`reasoning_bridge.rs:30-94`）。该实现解决了本地无状态 tool loop 的 replay 需要，但 envelope 只有固定前缀和 payload：没有 issuer、route/deployment binding、expiry、完整性保护或 replay policy。

因此 OpenBridge 不应直接复用此格式，也不应把任何 upstream-signed/provider-issued field 当作 proxy 可以自行重新签发的普通 JSON。应沿用既定 `provider_data` envelope：明确 `issuer`、`protocol`、`replay_policy` 与 payload；仅当 issuer 与实际 endpoint 一致且 capability 允许时 replay，并对丢弃或拒绝写 `ConversionNotice`。

## 7. 对 OpenBridge C3/C4 的落地建议

| 优先级 | 采用项 | 目标模块/边界 | 不采用项 |
|---|---|---|---|
| P1 | per-request `ToolConversionContext` 和 Responses item 顺序 renderer | 新建 `protocol/` / `pipeline/` 转换层；不放 ingress | provider name/model 猜测 |
| P1 | `call_id`、`item_id`、`output_index` 分离的 stream assembler | Chat->Responses 与 Responses->Chat 各自 iterator | 用一个 mapper 同时处理 final JSON 与 SSE |
| P1 | fragmented tool args、并行 calls、late id/name、tool-result adjacency fixtures | C3/C4 fixture/contract tests | 成功时静默删除无效或未知 tool |
| P2 | issuer-bound continuation ledger，仅为确有 Chat 上游相邻项约束的 route 启用 | control/pipeline 外围状态；TTL 和容量必须配置 | cc-switch 的全局 unique-`call_id` fallback |
| P2 | tool schema normalization 作为特定 provider adapter 的显式规则 | `ProviderAdapter` + `CapabilityProfile` | 在 Bridge IR 中硬编码第三方 gateway quirks |
| P3 | reasoning/item replay 条件与转换 notice | `ContinuationRef` / `ConversionNotice` | Base64 伪装为 provider signature 的无绑定 envelope |

建议的最小实现切片不变：先只支持文本 + function tool schema/call/result 的双向非流式转换，再分别实现两个 SSE iterator，最后才引入 continuation 和非 function built-in tools。cc-switch 的大体量模块说明这些 concerns 最终会增长，但不应在 OpenBridge 初期复制其桌面应用、计费、provider fallback 或私有协议兼容分支。

## 8. 由 cc-switch 补充的 C3/C4 fixture 清单

在现有 [转换设计的测试要求](../../design/chat-responses-conversion.md#13-测试性质) 之外，至少补充：

1. **连续与并行 tool calls**：多个 `function_call` 先合并为一个 Chat assistant message；每个 `function_call_output` 仍按原 `call_id` 关联。
2. **fragmented Chat tool delta**：id、name、arguments 分多帧到达；后续空字符串 fragment 不得擦除先前身份。
3. **乱序到达的并行 index**：不因后一个 call 更早完整而改变 Responses `output_index` 顺序。
4. **custom/namespace tools**：若某 route 声明支持降级，断言 request context 能准确反向恢复；否则明确拒绝并有 notice。
5. **tool schema 边界**：`null`、缺失、非 object、`oneOf` 等 schema 仅在对应 provider profile 的允许规则下规范化，并记录是否有损。
6. **continuation recovery**：相同 issuer/deployment 的 `previous_response_id` 能补回 call group；不同 issuer/deployment、过期、缺失或歧义记录一定不可猜测重放。
7. **stream terminal**：`output_item.done` 不结束；flush arguments 后只发一个 terminal；上游 transport error、EOF-before-terminal 与 client cancellation 有可区分结果/诊断。
8. **notice/audit**：每个 builtin-tool drop、status approximation、schema normalization、continuation refusal 都能断言 machine-readable `ConversionNotice` 与 metadata-only audit observation。

## 相关资源

- [Chat/Responses 转换设计](../../design/chat-responses-conversion.md)
- [Rust provider adapter 与数据流](../../architecture/rust-provider-adapter-dataflow.md)
- [实施阶段索引](../../phases/README.md)
- [OpenAI Responses 协议](../../specifications/openai/responses-protocol.md)
- [Codex OAuth 与工具调用分析](../codex/oauth-and-tool-call-analysis.md)
