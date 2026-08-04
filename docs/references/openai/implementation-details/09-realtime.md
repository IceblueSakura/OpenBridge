# Realtime 协议实现细节

**目标状态：** 仅作协议参考，不在现阶段 1/2 实施范围。

## 范围与 transport

Realtime 是双向长连接协议族。HTTP endpoint 可创建 session/client secret 或管理 call，但实际媒体和事件通过 WebRTC、WebSocket 或 SIP 交互；它不能套用当前 HTTP JSON/SSE transport。

官方资料：[Realtime and audio](https://developers.openai.com/api/docs/guides/realtime)、[WebRTC](https://developers.openai.com/api/docs/guides/realtime-webrtc)、[WebSocket](https://developers.openai.com/api/docs/guides/realtime-websocket)、[Realtime conversations](https://developers.openai.com/api/docs/guides/realtime-conversations) 与 [Realtime transcription](https://developers.openai.com/api/docs/guides/realtime-transcription)。

需要分别建模：

- voice-agent session；
- realtime transcription session；
- realtime translation session；
- WebRTC call 与 server-side controls；
- ephemeral client secret；
- 双向 client/server events、audio buffers 和 response lifecycle。

## 为什么不能复用 SSE

SSE 是服务器到客户端的单向 HTTP response stream；Realtime 同一连接上同时接收 client events、发送 server events、传输音频并维护 session/conversation state。其语义包括 session update、audio buffer append/commit/clear、conversation item、response create/cancel、VAD 和多种 delta/terminal events。

实现至少需要新的 transport contract：连接建立、双向 backpressure、heartbeat/idle timeout、frame/event limits、并发 send/receive、关闭码、取消、重连和 session state。`HttpJsonSse`、首个 response byte commit 和 Responses SSE terminal 只能作为部分经验，不能作为兼容证据。

## client secret 与认证代理

官方 session/client-secret endpoint 可生成短期 token 给浏览器或移动客户端，避免暴露主 API key。OpenBridge 若转发该能力，必须决定：

- 下游静态用户如何绑定 ephemeral secret、session 和终端用户 safety identifier；
- secret TTL、scope、一次/多次使用和日志脱敏；
- 浏览器是直连上游，还是所有媒体经 OpenBridge 中继；
- 直连上游时如何避免暴露内部 Provider/Target 和破坏 gateway 观测/路由；
- 中继时如何处理 WebRTC SDP/ICE/TURN、网络 egress 和媒体带宽。

返回 upstream ephemeral token 会让客户端绕过 OpenBridge 后续请求路径；它不是普通 JSON response passthrough。没有明确威胁模型前，应默认不开放。

## session affinity 与 reconnect

session/call ID、conversation items、audio buffer 和 response IDs 都绑定 issuing Target/API。连接建立后不允许跨 Provider fallback；断线重连是否可恢复、可恢复哪些 item、是否需要完整历史，都必须来自具体 Provider 协议证据。

新 session 可以按静态 Route 选择健康 candidate；已有 session 不能因 cooldown 迁移。若 provider 失败，只能按协议关闭/报告，不能把未完成音频和事件拼到另一 session。

## 媒体、安全与观测

- 明确输入/输出 audio format、sample rate、channels、frame/chunk size 和 base64/binary framing；不做隐式转码。
- 限制单 event、单 audio chunk、累计 buffer、session duration、idle time 和并发 session 数。
- 不记录原始音频、transcript、instructions、ephemeral secret、SDP、ICE credential 或完整 event payload。
- WebRTC/SIP/public webhook 会扩大当前 loopback 信任边界，需要独立网络与部署需求，不能作为普通 route 增量。
- 对浏览器 voice agent 的用户披露、录音同意、speaker/voice policy 和数据保留需单独验收。

## TDD 与验收矩阵

| 层 | 必须证明 |
|---|---|
| handshake | WebSocket/WebRTC authentication、session config、TTL、关闭码和 secret redaction |
| events | client/server event schema、ordering、duplicate/unknown event、terminal 与 cancel |
| audio | format/chunk/total limits、backpressure、append/commit/clear 和下游取消 |
| affinity | session/call/response ID 固定 issuer，重连不跨 Target |
| resilience | connect 前有限选择；连接后不 fallback；disconnect/timeout 唯一终态 |
| browser/network | CORS/origin、SDP/ICE/TURN、直连或中继边界及安全 header |

单元测试和本地 mock 只能证明 event state machine。真实 WebRTC/WebSocket、网络穿透、设备音频、延迟、长时间连接和 Provider session recovery 都属于额外验收层。

## 非目标

- 不把 `/audio/speech` 或 transcription SSE 称为 Realtime；
- 不把 Realtime events 转成 Chat/Responses SSE；
- 不在未定义威胁模型时把 upstream ephemeral secret 直接交给下游；
- 不承诺跨 Provider session migration、媒体转码、电话/SIP 或 TURN 服务；
- 现阶段 1/2 实施范围不包含本协议。
