# Codex Responses SSE 与工具调用生命周期调研

## 状态与证据

| 项目           | 值                                                                                                  |
|----------------|-----------------------------------------------------------------------------------------------------|
| 调研仓库       | `openai/codex`                                                                                      |
| 原始逐行快照   | `4c43465133428898aa84f0bfc02c306ed65fb66a`，2026-07-25                                              |
| 当前模块级复核 | `ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff`，2026-08-01                                              |
| 阅读范围       | `codex-rs/codex-api` 的 Responses HTTP/SSE parser，`codex-rs/core` 的 event、tool 与 TTFT lifecycle |
| 排除           | OAuth、订阅 backend、TUI、审批、sandbox、hook 和 Provider catalog                                   |

当前提交仍可定位 `process_responses_event()`、`ToolCallInputDelta`、`ResponseEvent::Completed`、`x-codex-turn-state` 与
`supports_websockets`。原始细粒度行号不作为当前提交定位。

## 1. HTTP 与 SSE 分层

Codex Responses client 显式请求 `text/event-stream`。HTTP response 到达后，client 先读取模型、rate-limit、etag 和 reasoning
metadata，再将 body 交给独立 SSE processor。

SSE processor 不把每个 `data:` 当普通文本，而是按 event `type` 映射为类型化 `ResponseEvent`。这使 transport/framing 与
Responses semantic event 成为不同层次。

## 2. Event 与 terminal

固定快照中的 parser 可区分：

- text 与 reasoning summary/content delta；
- output item added/done；
- function/custom tool input delta；
- response completed、failed 与 incomplete；
- error 与未知 event。

`response.output_item.done` 只结束一个 item；`response.completed` 才产生成功 response terminal。parser 对部分未知 event
采用忽略/记录策略，但该策略只说明 Codex 当前 consumer 的兼容选择。

## 3. 私有 metadata 与 turn state

Codex 可以从 HTTP header 或 `response.metadata.headers` 取得 `x-codex-turn-state`，并在同一 turn 后续请求回传，以维持
sticky routing。

这是 Codex product profile 的私有 state。公开 Responses schema 没有因此获得同名标准字段；该 state 的 issuer、生命周期和可转移性不能从
client cache 行为推导。

## 4. Tool identity 与执行时序

Codex 维护 item id、call id、tool name、arguments 和 output 的独立关联：

- custom tool delta 保留 item identity 与可选 call identity；
- fragmented arguments 在 item lifecycle 内累计；
- function/custom call 完成后可在 response terminal 前开始本地工具执行；
- parallel tool tests 断言多个调用的启动与 output 按 `call_id` 回接；
- cancel 与 terminal 到达会影响仍在运行的本地 tool task。

本地 tool execution、approval 和 sandbox 属于 Codex Agent runtime，不是 Responses server 的 wire responsibility。

## 5. Codex 的 TTFT 语义

Codex core 的 TTFT 观察更接近“收到第一个模型业务事件”，而不是 socket 首字节。text、reasoning 与 tool delta 是否计入需要看具体
event handler；`response.created` 或纯 lifecycle event 不等同于首个可消费模型输出。

因此引用 Codex TTFT 时必须给出事件条件，不能只写一个未定义的 `TTFT` 名称。

## 6. WebSocket 与兼容边界

`ModelProviderInfo` 存在 `supports_websockets` 且默认 false。这说明 Codex custom Provider profile 可声明 transport
能力，但字段存在本身不证明任意 endpoint 兼容 Responses WebSocket。

本调研也不证明：

- Codex 当前 event 集合等于完整或长期稳定的 OpenAI Responses API；
- private header、模型 metadata、rate-limit 或 telemetry 应由其他服务公开；
- 一个 Codex stream fixture 同时证明其他 Agent/SDK 兼容；
- Codex OAuth/client identity 与 Responses wire contract 是同一证据。

## 一手源码

- [
  `codex-api/src/sse/responses.rs`](https://github.com/openai/codex/blob/ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff/codex-rs/codex-api/src/sse/responses.rs)
- [
  `core/tests/common/responses.rs`](https://github.com/openai/codex/blob/ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff/codex-rs/core/tests/common/responses.rs)
- [Codex protocol test assets](codex-protocol-test-assets-analysis.md)
- [Codex device auth and refresh](codex-device-auth-token-refresh-analysis.md)
