# M04 原生转发与流式处理

## Native Path

当下游和上游协议一致时：

- 只解析路由和 capability 所需字段；
- 只改写 `model` 等明确允许字段；
- 不进入 Bridge IR；
- 尽量保留未知合法 JSON 字段和上游 wire bytes。

## SSE 规则

- 网络 chunk 不等于 SSE event；
- 支持分片 UTF-8、CRLF、多 event 同 chunk、event 跨 chunk 和多行 `data:`；
- Chat 以 `[DONE]` 识别完成；
- Responses 区分 `completed`、`failed` 和 `incomplete`；
- EOF without terminal 不伪造成功；
- partial output 后不 retry/fallback；
- 下游取消应释放上游 stream。

## 当前状态

本地 unit/contract/SDK loopback 已覆盖：

- Chat/Responses stream 与 non-stream；
- SSE framing 和 event size；
- 429/5xx/timeout 的首输出前处理；
- partial stream failure；
- cancel propagation；
- 安全响应 header。

仍需真实 Codex/Hermes tool loop 和真实/脱敏 Provider corpus。

## 详细资料

- [当前实现](../implementation/current-implementation.md)
- [Provider 韧性需求](../requirements/provider-resilience.md)
- [目标架构](../architecture/architecture-and-roadmap.md)
- [Responses 协议](../specifications/openai/responses-protocol.md)
- [Chat Completions 协议](../specifications/openai/chat-completions-protocol.md)
