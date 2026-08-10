# OpenAI Realtime 双向 transport 调研

## 来源、范围与快照

本文只记录 Realtime WebRTC/WebSocket/SIP data plane 的双向 media 与 typed event session。HTTP control plane、request-based Audio API
和普通 Responses SSE 不在本文定义。

- 官方来源：[Realtime](https://developers.openai.com/api/docs/guides/realtime)、[WebRTC](https://developers.openai.com/api/docs/guides/realtime-webrtc)、[WebSocket](https://developers.openai.com/api/docs/guides/realtime-websocket)、[SIP](https://developers.openai.com/api/docs/guides/realtime-sip)、[Realtime translation](https://developers.openai.com/api/docs/guides/realtime-translation)、[Realtime transcription](https://developers.openai.com/api/docs/guides/realtime-transcription)；
- 官方资料复核日期：2026-08-10；动态 event、model、session field 与 media format 使用前仍须重核。

## 1. Session types

| Session purpose | Data-plane entry/identity | 关键语义 |
|-----------------|---------------------------|----------|
| Voice agent/conversation | `/v1/realtime` | conversation item、audio buffer、response、tool 与 typed audio/text events |
| Live translation | `/v1/realtime/translations` | 连续解释输入并输出翻译 audio/text；不使用 `response.create` 驱动每轮响应 |
| Live transcription | Realtime session 配置 `type: "transcription"` | 输出 transcript delta/completed events，不创建普通 assistant spoken response |

这三种 session 共享部分 Realtime event/transport 概念，但不是同一个状态机开关。兼容实现必须分别固定 session schema、允许的 client
events、server events、terminal/close 语义与 output ownership。

## 2. Transport entries

| Transport | 当前入口或 signaling | 部署边界 |
|-----------|----------------------|----------|
| WebSocket | `wss://api.openai.com/v1/realtime?model=...` | 面向可信 backend；长连接内传 typed JSON events 与编码 audio |
| WebRTC | 对话/转写使用 `/v1/realtime/calls`；翻译使用 `/v1/realtime/translations/calls` | 浏览器/移动端优先；media channel 与 data channel 由 WebRTC 协商 |
| SIP | `sip:$PROJECT_ID@sip.api.openai.com;transport=tls` | 电话入口结合 incoming-call webhook 与 call control operations |

WebRTC 的 unified SDP handshake 及 SIP call action 属于 [HTTP control plane](control-plane.md)；列在这里是为了固定 data-plane 建立方式，
不把 signaling response 误当普通 JSON API。

## 3. 与 SSE 的差异

Realtime 是长连接双向 protocol，不是 HTTP request + 单向 SSE response。reconnect、buffer ownership、backpressure、partial media、
cancel 与 session state 都是 transport state machine 的一部分。

WebRTC、WebSocket 与 SIP/telephony 也不能压缩为同一个 transport flag；各自 signaling、media channel 与部署拓扑需单独验证。

## 4. Media、tool 与安全边界

binary/Base64 audio、text、transcript 与 tool event 可共享 session，但拥有不同 buffer 与数据敏感度。audio content、transcript、tool
arguments 和 session metadata 不应进入普通日志。

## 5. 证据边界

- `/audio/speech` 或 transcription success 不证明 Realtime；
- Responses SSE terminal 不适用于 Realtime session close；
- voice-agent session success 不证明 translation 或 transcription session；
- 单个 WebSocket sample 不证明 WebRTC/SIP、重连、长时 backpressure、VAD 或生产负载。
