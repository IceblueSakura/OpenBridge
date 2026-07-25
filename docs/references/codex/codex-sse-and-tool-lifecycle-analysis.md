# Codex Responses SSE 与工具调用生命周期调研

## 状态与范围

**外部实现调研；不代表 OpenBridge 已实现或承诺完整 Codex 兼容。**

| 项目 | 值 |
|---|---|
| 调研仓库 | `https://github.com/openai/codex` |
| 本地快照 | `F:/codespace/codex`，`main` @ `4c43465133428898aa84f0bfc02c306ed65fb66a` |
| 快照日期 | 2026-07-25 |
| 阅读范围 | `codex-rs/codex-api` 的 Responses HTTP/SSE 入口与 parser，`codex-rs/core` 的事件消费、tool 生命周期与 TTFT 记录 |
| 矩阵角色 | 本地 Codex 的 Responses 下游契约与 Rust 实现主参考 |
| 不在范围 | OAuth/client identity、auth cache、订阅 backend、CLI/TUI、审批、sandbox、hook 或 Provider catalog |

本文件补充[Codex OAuth 安全边界](codex-oauth-and-tool-call-analysis.md)：后者只说明 OAuth 不可外推；本文只研究 OpenBridge 可用于 HTTP/SSE、事件终态、工具关联与测试的客户端行为。

## 1. 可复用结论

| 观察 | 当前源码证据 | 对 OpenBridge 的约束 |
|---|---|---|
| HTTP Responses stream 显式请求 `text/event-stream`，再进入独立 SSE 处理 | `codex-api/src/endpoint/responses.rs:128-167` | Native Path 必须把 HTTP/SSE transport 与协议解释分开；支持原生转发时不应重渲染未知合法 event。 |
| SSE parser 将 wire event 映射为类型化 `ResponseEvent`，而非把所有 `data:` 当文本 | `codex-api/src/common.rs:76-123`，`sse/responses.rs:327-466` | 测试至少区分 text、reasoning、item、tool delta、completed、failed/incomplete，而非只断言连接未断开。 |
| `response.output_item.done` 是 item 生命周期事件，`response.completed` 才产生 `Completed` | `sse/responses.rs:327-438` | bridge 和统计不能把 item done 当成请求成功终态。 |
| custom tool input delta 保留 `item_id` 与可选 `call_id` | `sse/responses.rs:344-353` | `item_id`、`call_id`、stream/output index 是不同身份；不可互相替换。 |
| `x-codex-turn-state` 在同一 turn 被回传以维持 sticky routing，且可来自 HTTP header 或 `response.metadata.headers` | `core/src/client.rs:11-16, 267-283, 1887-1903`；`sse/responses.rs:62-68, 203-211` | 只在受限的 Codex Native Responses profile 中双向透明保留；不能生成、记录、跨 deployment/bridge/fallback 重放。 |
| Core 在 item 完成和 stream 完成之间继续处理工具与取消 | `core/src/session/turn.rs:2113-2349` | OpenBridge 只维护 wire-level tool call/result，不接管 Codex 的审批、工具执行或 sandbox。 |

## 2. HTTP/SSE 处理形状

`ResponsesClient::stream_encoded()` 固定以 `POST responses` 调用 transport，并写入 `Accept: text/event-stream`。得到 HTTP stream 后，`spawn_response_stream()` 先从 response header 提取模型、rate-limit、etag、reasoning 相关元数据，再创建容量为 1600 的事件 channel，并调用 `process_sse_with_treatment()`（`codex-api/src/sse/responses.rs:33-97`）。

这说明两个对 OpenBridge 有用、但不可机械复制的分层：

```text
HTTP response headers + byte stream
→ SSE framing / event JSON
→ typed ResponseEvent
→ Agent session、tool runtime、telemetry
```

OpenBridge 目前 Native Path 保留原始 SSE bytes，同时只做 framing 验证；这与 Codex 的“客户端解析为内部事件”不冲突。只有进入 Protocol Bridge、调用统计或明确的 protocol conformance fixture 时，才需要解析并区分 event 语义。

## 3. event、终态与未知输入

当前 `process_responses_event()` 至少处理：

- `response.created`；
- `response.output_item.added`、`response.output_item.done`；
- `response.output_text.delta`；
- reasoning summary/content delta；
- `response.custom_tool_call_input.delta`；
- `response.completed`、`response.failed`、`response.incomplete`。

`response.completed` 会反序列化 response id、usage 和可选 `end_turn` 为 `ResponseEvent::Completed`。`response.failed` 与 `response.incomplete` 转为错误结果，前者还会把可识别的上下文窗口、quota、策略、invalid request、过载或 retryable 情形细分（`sse/responses.rs:386-438`）。未知 event 仅 trace 记录后忽略。

对 OpenBridge 的结论不是复制其容错策略：

- Native Path 可以保留未知合法 event 的 wire bytes；
- Bridge Path 必须对每个未映射 event 明确 `mapped`、`rejected` 或带损失说明地处理，不能仅因 Codex 当前忽略就静默丢弃；
- 已经向下游写出 body 后的 `failed`、`incomplete`、EOF 或 parser 错误属于当前 stream 的终态，不能进入 retry/fallback。

### 3.1 `response.metadata` 与私有 header

Codex 还识别 `response.metadata`，从中读取 `headers`、`openai_verification_recommendation` 和 `openai_chatgpt_moderation_metadata`；它也会读取 HTTP response 的 `openai-model`、`x-reasoning-included`、`x-request-id`、rate-limit、models etag 与 `x-codex-turn-state`（`sse/responses.rs:28-68, 203-303, 539-593`）。其中 `x-codex-turn-state` 被 Core 明确注释为同一 turn 的 sticky-routing token，并会在后续请求回传。

这不是公开 Responses wire contract 的自动扩张。详细分界和 header policy 见[Responses 协议的 Codex 交叉核对](../openai/responses-protocol.md#62-codex-交叉核对标准事件与私有扩展分界)与[私有 header 规则](../openai/responses-protocol.md#63-codex-私有-header-与同一路径续接)。本调研确认的 OpenBridge 要求只有：

- 受显式 allowlist 保护的 Native Path 同向透明保留 `x-codex-turn-state`，不解析 token 内容；
- `response.metadata`、审核/验证信息和其他私有 header 不能塞入 Bridge IR、普通 Responses `metadata`、下游 Chat SSE 或用户可见 transcript；
- 其他 `x-codex-*` 名称尚未在本次 HTTP Responses 流核对中证明必须透传，不能据此放宽为通配 header forwarding。

## 4. 工具关联与执行边界

`ResponseEvent` 的 `ToolCallInputDelta` 同时携带 `item_id`、可选 `call_id` 和增量文本。Core 在 `OutputItemAdded(CustomToolCall)` 时用 `call_id` 创建 argument diff consumer；在 `OutputItemDone` 时结束 consumer，随后把完成 item 交给自身的 tool runtime（`core/src/session/turn.rs:2113-2239`）。

由此可确认：

1. `call_id` 是 tool call/output 的关联键，不能拿 item id、输出序号或函数名代替；
2. fragmented argument 需要按 call/item 状态累积，空或晚到 fragment 不应覆盖先前身份；
3. Codex 的本地 tool 执行发生在客户端 runtime。OpenBridge 不应因为观察到这个流程而执行 Agent 返回的 function/custom tool；它只需可靠地转发或在 bridge 中重建 wire-level call/result。

建议的 OpenBridge fixture：并行 call、late/empty id or name、fragmented arguments、item done 早于 completed、`response.failed`、`response.incomplete`、EOF-before-terminal、下游取消，以及 `response.metadata.headers` / HTTP header 携带 turn state 时的 native preserve 与跨 deployment 拒绝。

## 5. Codex 的 TTFT 不是 OpenBridge 的通用口径

Codex 存在至少两层内部计时：

- `core/src/client.rs:1962-2045` 在 stream wrapper 首次收到 `OutputItemAdded` 时记录 `ttft_ms`，并在 `Completed` 时把 usage 与该值交给 session telemetry；
- `core/src/turn_timing.rs:360-394` 将非空 message/reasoning item 或其 delta 视为 turn TTFT，但排除 `Created`、tool input delta、`Completed`、rate-limit 等事件。

这证明“TTFT”必须写明事件语义，而不是只有一个名字。它**不**改变 OpenBridge 已定义的网关口径：流式 `gateway_ttft_ms` 计到网关成功写出的首个 response body byte；非流式单列 `gateway_ttfb_ms`。Codex 的语义可作为额外的、协议感知的观测样本，不能替代网关端到端计时，也不能用 `response.created` 或 tool delta 冒充首个模型输出。

## 6. 目标客户端验证与非结论

Codex 的 `ModelProviderInfo` 仍有 `supports_websockets` 字段且默认 false（`model-provider-info/src/lib.rs:140-160`）；这只说明 custom Provider 配置有该能力开关。OpenBridge 初期保持显式 `supports_websockets = false`、验证 HTTP/SSE custom Provider profile；字段存在不构成 Responses WebSocket 的兼容承诺。

本调研不证明：

- 当前 Codex 解析的事件集合等于完整或长期稳定的 OpenAI Responses API；
- 当前 client 内部的 header、模型 metadata、rate-limit 或 telemetry 结构应被 OpenBridge 对外暴露；
- Codex OAuth、`auth.json`、客户端身份或 subscription route 可供 OpenBridge 使用；
- Codex 通过的 stream 就能证明 Hermes 或其他 Agent 已兼容。

## 相关资源

- [项目比较矩阵](../project-comparison.md)
- [Codex OAuth 安全边界](codex-oauth-and-tool-call-analysis.md)
- [OpenAI Responses 协议](../openai/responses-protocol.md)
- [调用统计与可观测性需求](../../functional-requirements/observability.md)
- [客户端兼容计划](../../implementation-plans/client-compatibility.md)
