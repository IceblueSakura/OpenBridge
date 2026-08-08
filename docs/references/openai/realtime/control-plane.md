# OpenAI Realtime HTTP control plane 调研

## 来源、范围与快照

本文只记录 Realtime session/client-secret/call 等 HTTP control-plane resource。WebRTC/WebSocket/SIP media/event transport 由另一文档
维护。

- 官方来源：[Realtime](https://developers.openai.com/api/docs/guides/realtime)、[Realtime WebRTC](https://developers.openai.com/api/docs/guides/realtime-webrtc)
- 官方资料复核日期：2026-08-08；动态 endpoint、secret schema、TTL 与 scope 使用前仍须重核。

## 1. Session 与 short-lived credential

官方资料包含为浏览器/移动客户端创建短期 client secret，以及建立 Realtime call/session 的 HTTP control operation。secret 的
audience、TTL、scope 与泄露影响必须分别记录，不能当作普通长期 API key。

## 2. Topology

客户端直连上游与中间服务中继全部 media 是不同拓扑。control plane 只负责建立/授权 session；它不证明 data-plane event、audio
buffer、backpressure 或 reconnect 已兼容。

## 3. Identity 与安全

session/call/client-secret identity 不能由 Responses `response_id` 或 audio file id 推导。ephemeral secret、session metadata 与
signaling data 不应写入日志或 fixture。

## 4. 证据边界

- HTTP session 创建成功不证明 WebRTC/WebSocket media 可用；
- 一种 client topology 不证明另一种 relay/直连 topology；
- mock secret 不证明真实 TTL、scope、rotation 或跨账户权限。
