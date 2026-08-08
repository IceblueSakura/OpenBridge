# OpenAI Realtime 双向 transport 调研

## 来源、范围与快照

本文只记录 Realtime WebRTC/WebSocket/SIP data plane 的双向 media 与 typed event session。HTTP control plane、request-based Audio API
和普通 Responses SSE 不在本文定义。

- 官方来源：[Realtime](https://developers.openai.com/api/docs/guides/realtime)、[WebRTC](https://developers.openai.com/api/docs/guides/realtime-webrtc)、[WebSocket](https://developers.openai.com/api/docs/guides/realtime-websocket)、[Realtime conversations](https://developers.openai.com/api/docs/guides/realtime-conversations)、[Realtime transcription](https://developers.openai.com/api/docs/guides/realtime-transcription)
- 官方资料复核日期：2026-08-08；动态 event、model 与 media format 使用前仍须重核。

## 1. Session/event model

client 与 server 围绕 session、conversation item、audio buffer、response、tool 与 typed audio/text event 双向交互。session update、audio
append/commit、VAD、response create/cancel 与 tool output 可交错。

## 2. 与 SSE 的差异

Realtime 是长连接双向 protocol，不是 HTTP request + 单向 SSE response。reconnect、buffer ownership、backpressure、partial media、
cancel 与 session state 都是 transport state machine 的一部分。

WebRTC、WebSocket 与 SIP/telephony 也不能压缩为同一个 transport flag；各自 signaling、media channel 与部署拓扑需单独验证。

## 3. Media、tool 与安全边界

binary/Base64 audio、text、transcript 与 tool event 可共享 session，但拥有不同 buffer 与数据敏感度。audio content、transcript、tool
arguments 和 session metadata 不应进入普通日志。

## 4. 证据边界

- `/audio/speech` 或 transcription success 不证明 Realtime；
- Responses SSE terminal 不适用于 Realtime session close；
- 单个 WebSocket sample 不证明 WebRTC/SIP、重连、长时 backpressure、VAD 或生产负载。
