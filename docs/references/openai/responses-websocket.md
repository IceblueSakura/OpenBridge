# OpenAI Responses WebSocket mode 调研

## 来源、范围与快照

本文只记录 Responses API 的 persistent WebSocket mode。HTTP JSON、HTTP SSE、Realtime WebSocket 和 resource CRUD 分别由其他 owner
文档维护。

- 官方来源：[Responses WebSocket mode](https://developers.openai.com/api/docs/guides/websocket-mode)、
  [Responses WebSocket events](https://developers.openai.com/api/reference/resources/responses/websocket-events)；
- 官方资料复核日期：2026-08-10；connection limit、event type、warmup、cache 和 error code 使用前仍须重核。

## 1. Connection 与 request event

当前官方入口为：

```text
wss://api.openai.com/v1/responses
Authorization: Bearer ...
```

客户端在连接内发送 `response.create` JSON event。其 payload 大体复用 Responses create body，但 WebSocket transport 不使用 HTTP
create 中的 `stream` 与 `background` 字段。可选的 `generate: false` warmup 会准备 request state 并返回可继续引用的 response id，
但不生成普通 model output。

## 2. Continuation 与 connection-local state

后续 turn 发送新的 `response.create`，使用前一 `previous_response_id`，并只携带新增 input items。active connection 当前缓存最近一个
previous-response state；该内存状态允许 `store: false`/ZDR 下的低延迟 continuation。

若 id 不在 connection-local cache：

- `store: true` 时，服务可能从仍可用的持久状态恢复，但不再具有同样的内存快路径；
- `store: false`/ZDR 时没有持久 fallback，会产生 `previous_response_not_found`；
- turn 发生 `4xx`/`5xx` 时，官方当前行为会从 connection cache 驱逐该失败 continuation 引用的 previous response。

这不是普通 route retry：response id、连接与 storage issuer 必须保持一致。

## 3. Event、并发与重连

server event 与顺序沿用 Responses streaming event model，但 framing 是 WebSocket message，不是 SSE `event:`/`data:`。当前快照中：

- 一个 connection 可接受多个 `response.create`，但顺序执行，同一时刻只有一个 in-flight response；
- 不支持 multiplexing，需要并行时使用多个 connection；
- connection duration 上限为 60 分钟，到期后必须重连；
- 重连后，已持久 response 可继续引用；无法恢复时需要以完整 context 开新 chain；
- 独立 `/v1/responses/compact` 返回 compacted input window，不返回 response id，后续应把该 window 作为新 chain input。

上述限制是动态服务事实，不应固化为跨 Provider 的永久常量；兼容实现应把它们作为有日期的 profile/limit。

## 4. 与 HTTP SSE 和 Realtime 的区别

Responses WebSocket 复用 Responses item/event 与 `previous_response_id` 语义，适合多轮 tool-heavy workflow；Realtime 则拥有独立的
session、audio buffer、client/server event 和 WebRTC/SIP transport。二者虽然都是 WebSocket，但不能共享一个通用“streaming”开关。

HTTP Responses SSE 是单个 request 的单向 response body。WebSocket mode 的 handshake、message boundary、顺序执行、连接缓存、重连和
close 都是新增状态机。

## 5. Fake 与真实证据边界

F4 fake 至少应验证：

- Bearer handshake、错误 upgrade 与连接关闭；
- `response.create` schema，以及 `stream`/`background` 等 HTTP-only transport field 的既定处理；
- server event ordering、terminal、tool round trip 和一次只执行一个 response；
- `store: false` connection-cache continuation、cache miss 和失败驱逐；
- compacted window、新 chain、重连与 connection limit；
- downstream close 向 upstream cancel/cleanup 的传播和 bounded backpressure。

loopback WebSocket 通过不能证明真实 Provider 提供该 mode、官方 60 分钟连接、长时稳定性、ZDR 合规、工具语义或生产负载。
