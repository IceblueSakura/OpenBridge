# OpenAI Realtime HTTP control plane 调研

## 来源、范围与快照

本文只记录 Realtime client-secret、call signaling 与 call action 等 HTTP control-plane operation。WebRTC/WebSocket/SIP media/event transport 由另一文档
维护。

- 官方来源：[Create client secret](https://developers.openai.com/api/reference/resources/realtime/subresources/client_secrets/methods/create)、[Create translation client secret](https://developers.openai.com/api/reference/resources/realtime/subresources/translations/subresources/client_secrets/methods/create)、[Realtime WebRTC](https://developers.openai.com/api/docs/guides/realtime-webrtc)、[Realtime translation](https://developers.openai.com/api/docs/guides/realtime-translation)、[Realtime SIP](https://developers.openai.com/api/docs/guides/realtime-sip)、[Realtime calls](https://developers.openai.com/api/reference/resources/realtime/subresources/calls/methods/accept)；
- 官方资料复核日期：2026-08-10；动态 endpoint、secret schema、TTL、scope 与 call action 使用前仍须重核。

## 1. Endpoint map

| Method | Path                                             | 当前 operation |
|--------|--------------------------------------------------|----------------|
| `POST` | `/v1/realtime/client_secrets`                    | 创建 voice-agent/general Realtime 短期 client secret |
| `POST` | `/v1/realtime/translations/client_secrets`       | 创建 live-translation 短期 client secret |
| `POST` | `/v1/realtime/calls`                             | WebRTC handshake：unified multipart 或持短期 secret 提交 SDP，返回 SDP answer |
| `POST` | `/v1/realtime/translations/calls`                | 持 translation client secret 提交 SDP，建立 translation WebRTC call |
| `POST` | `/v1/realtime/calls/{call_id}/accept`            | 接受 incoming SIP call |
| `POST` | `/v1/realtime/calls/{call_id}/reject`            | 拒绝 incoming SIP call |
| `POST` | `/v1/realtime/calls/{call_id}/hangup`            | 挂断已建立 call |
| `POST` | `/v1/realtime/calls/{call_id}/refer`             | 将 SIP call 转接到目标 URI |

client secret 是短期 credential，其 audience、TTL、scope 与泄露影响必须分别记录，不能当作普通长期 API key。voice-agent secret 与
translation secret 也不能仅因字段相似而互换。

## 2. Topology

客户端直连上游与中间服务中继全部 media 是不同拓扑。control plane 只负责建立/授权 session；它不证明 data-plane event、audio
buffer、backpressure 或 reconnect 已兼容。

## 3. Legacy boundary

截至快照日期，API Reference 仍在 **Legacy Realtime Beta** 分组列出 `POST /v1/realtime/sessions` 与
`POST /v1/realtime/transcription_sessions`。它们不是本文建议的新开发默认入口；只有明确兼容旧 beta client 时才应固定其 schema，
否则按当前 client-secret、unified call 与 session-type 文档实现。

## 4. Identity 与安全

session/call/client-secret identity 不能由 Responses `response_id` 或 audio file id 推导。ephemeral secret、session metadata 与
signaling data 不应写入日志或 fixture。

## 5. 证据边界

- HTTP session 创建成功不证明 WebRTC/WebSocket media 可用；
- 一种 client topology 不证明另一种 relay/直连 topology；
- mock secret 不证明真实 TTL、scope、rotation 或跨账户权限。
