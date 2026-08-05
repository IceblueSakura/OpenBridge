# cc-switch：Chat/Responses 与 Agent Tool 转换调研

## 状态与证据

| 项目           | 值                                                                                |
|----------------|-----------------------------------------------------------------------------------|
| 调研仓库       | `farion1231/cc-switch`                                                            |
| 原始逐行快照   | `08710d51fc04843ce217c58749677d84cf62740b`，2026-07-21                            |
| 当前模块级复核 | `ebbf141fc71547a99f669df1be8e345130d1d890`，2026-08-01                            |
| 阅读范围       | `src-tauri/src/proxy/providers/`、`forwarder.rs`、`handlers.rs`                   |
| 重点           | Codex Responses client 与 Chat Completions/Anthropic Messages upstream 之间的转换 |

当前提交仍可定位 `CodexToolContext`、`CodexChatHistoryStore`、`ChatToResponsesState` 与
`create_responses_sse_stream_from_chat_with_context`。下文的细粒度行号只属于原始快照。

## 1. 数据流

```text
Codex client /v1/responses
  -> choose configured upstream wire protocol
  -> build CodexToolContext from request tools and tool-search output
  -> enrich continuation from CodexChatHistoryStore when applicable
  -> convert Responses request to Chat request
  -> call upstream
  -> convert Chat JSON/SSE back to Responses objects/events
  -> persist eligible call items for a later continuation
```

该实现说明跨协议转换不仅需要字段映射，还需要每请求 tool context、跨请求关联状态和流式 lifecycle state。

## 2. Responses request 到 Chat messages

Responses input 是有序 item log。cc-switch 遍历原顺序，把相邻 function/custom/tool-search calls 组装为 Chat assistant
`tool_calls`，再把对应 outputs 转成 tool messages。

主要观察：

- tool result 依赖不变的 `call_id` 回接；
- 不能先按 role 分组再渲染，否则 assistant call 与 tool output 的相邻语义会改变；
- 多个相邻 calls 可以合并到一个 assistant message，但每个 call 的 identity 仍独立；
- 非相邻、缺失或歧义 output 不能仅按 tool name 猜测关联。

## 3. `CodexToolContext`

Responses 的 built-in/custom tool schema 不总能直接表示为 Chat function tool。cc-switch 在降级前建立每请求 context，保存原始
tool type、name、namespace、schema 和 tool-search 结果，使反向 response conversion 有机会恢复原 item kind。

这是一种有损转换补偿机制。当前源码还包含：

- 对非 object/缺失 schema 的规范化；
- 依据 Provider/model 名称选择兼容分支；
- 未知 tool type 的忽略路径；
- Anthropic Messages 与 Responses 的另一组转换规则。

这些行为是 cc-switch 的兼容策略，不是 OpenAI Chat/Responses 标准的一部分。

## 4. Continuation history

当客户端只发送 `previous_response_id + function_call_output`，而 Chat upstream 要求 assistant tool call 紧邻 tool result
时，`CodexChatHistoryStore` 保存前轮 call item 并在后续请求补回。

固定快照中的缓存与 fallback 观察包括：

- 以 response/call 相关 identity 查找历史；
- 对重复或不一致记录跳过/拒绝部分补全；
- 在找不到主要 key 时存在按唯一 `call_id` 回退；
- cache 是进程内产品状态，未展示 issuer、deployment、credential、TTL 与多节点一致性完整契约。

因此该 history store 只能说明 cc-switch 如何恢复其客户端上下文，不能证明 opaque continuation state 可跨 Provider 或
deployment 使用。

## 5. Chat SSE 到 Responses SSE

`ChatToResponsesState` 为 text、reasoning 与每个 tool call 分别维护状态。stream converter：

- 分配 response/item/output index；
- 按 Chat tool-call index 累积 id、name 和 arguments fragments；
- 输出 Responses item added、arguments delta/done、item done 等 lifecycle events；
- 在 finish/EOF 时 flush 尚未关闭的 text、reasoning 与 tool item；
- 最终产生 completed 或 incomplete 类 response terminal。

该状态机清楚展示 `output_item.done` 与 response terminal 是两个层次。它还会在已有实质输出但上游缺少 finish reason 时合成
incomplete 状态，这是 cc-switch 的 EOF recovery 策略。

## 6. Opaque reasoning 与私有数据

cc-switch 在 Anthropic/Responses 转换中处理 opaque reasoning/signature 类字段。某些路径为了跨协议 replay 会包装或重编码
provider data。

这些字段可能受原 issuer 签名、加密或验证，普通 JSON round-trip 不等于可以由另一个代理重新签发。项目实现只能说明其兼容处理方式，不能建立跨
Provider 的通用 reasoning contract。

## 7. 已覆盖与未覆盖边界

源码/tests 可提供以下场景：连续和并行 tool calls、fragmented arguments、tool schema 规范化、continuation 补全、stream item
lifecycle、EOF recovery 与 reasoning replay。

不能从该项目推导：

- 任意 Provider/model heuristic 都具有通用正确性；
- 未知 tool 可以安全静默丢弃；
- `call_id` 唯一性足以跨 issuer/deployment 恢复历史；
- 桌面 UI、usage、OAuth、client configuration takeover 或 Provider failover 属于协议转换要求。

## 一手入口

- [
  `transform_codex_chat.rs`](https://github.com/farion1231/cc-switch/blob/ebbf141fc71547a99f669df1be8e345130d1d890/src-tauri/src/proxy/providers/transform_codex_chat.rs)
- [
  `streaming_codex_chat.rs`](https://github.com/farion1231/cc-switch/blob/ebbf141fc71547a99f669df1be8e345130d1d890/src-tauri/src/proxy/providers/streaming_codex_chat.rs)
- [
  `codex_chat_history.rs`](https://github.com/farion1231/cc-switch/blob/ebbf141fc71547a99f669df1be8e345130d1d890/src-tauri/src/proxy/providers/codex_chat_history.rs)
