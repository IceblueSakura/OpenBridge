# Responses WebSocket 来源与边界

一手来源：[WebSocket mode](https://developers.openai.com/api/docs/guides/websocket-mode)、[WebSocket events](https://developers.openai.com/api/reference/resources/responses/websocket-events)。设计使用的来源版本见[固定基线](../upstream-sync.md)；连接额度、事件形状和服务端状态按具体任务重新核对，不在这里保留旧协议快照。

WebSocket 是独立的连接与资源生命周期合同，不是把 HTTP SSE 换成另一种 framing。设计相关能力前需要分别明确：

- create/control envelope 与 HTTP 请求的差异；
- lane、并发和 response 后继关系，与单 response reducer 的所有权分离；
- continuation、opaque state 的 issuer/auth scope 和有效期；
- 重连、取消、背压、资源预算与终态的语义；
- request、response、event 的独立预期，不能从事件名猜出整个会话合同。

本地实施缺口见 [Generation](../../implementation-status/generation.md)，现有 HTTP/SSE 测试不证明 WebSocket 可用。此页不保存调研过程或连接测试结果。
