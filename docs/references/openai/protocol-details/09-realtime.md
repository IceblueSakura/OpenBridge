# OpenAI Realtime 协议调研

## 1. Transport 与 session

Realtime 使用 WebRTC 或 WebSocket 等双向长连接 transport，围绕 session、conversation item、audio buffer、response 和 tool event 交互。它不是 HTTP request + SSE response 的单向生命周期。

资料：[Realtime](https://developers.openai.com/api/docs/guides/realtime)、[WebRTC](https://developers.openai.com/api/docs/guides/realtime-webrtc)、[WebSocket](https://developers.openai.com/api/docs/guides/realtime-websocket)、[Conversations](https://developers.openai.com/api/docs/guides/realtime-conversations)、[Transcription](https://developers.openai.com/api/docs/guides/realtime-transcription)。

## 2. 与 SSE 的差异

- client 和 server 都持续发送 typed events；
- session update、audio append/commit、VAD、response create/cancel 与 tool output 可交错；
- reconnect、buffer ownership 和 backpressure 是连接状态的一部分；
- binary/base64 audio 与 text/tool event 共享 session，但具有不同资源预算。

## 3. Client secret 与直连

官方资料包含为浏览器/移动客户端创建短期 client secret 的模式。客户端直连上游与由中间服务中继所有 media 是两种不同拓扑；短期 token 的 audience、TTL、scope 与泄露影响需要分别记录。

## 4. 边界

- Realtime session identity 不能从普通 Responses `previous_response_id` 推导。
- WebRTC、WebSocket 和 SIP/telephony 能力不能只用同一 transport flag 概括。
- 长连接取消、超时、重连和 tool output 需要独立状态机。
- audio content、transcript、ephemeral secret 和 session metadata 都可能敏感。
- 一次性 `/audio/speech` 或 transcription endpoint 不能证明 Realtime 兼容。

