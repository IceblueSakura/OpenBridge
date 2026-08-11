# 网关 API 域

本域只定义下游 API、认证、HTTP/SSE、tool 与 state 的可观察行为和失败边界。运行期观测由独立的
[观测域](../observability/README.md)拥有；已实现和验证范围只见[实施现状](../../implementation-status/README.md)。

| 功能模块 | 文档 |
|---|---|
| Endpoint 与认证 | [endpoints-and-auth.md](endpoints-and-auth.md) |
| MCP dual-era 本地服务 | [mcp.md](mcp.md) |
| 请求、Public Model 与安全边界 | [request-and-security-boundary.md](request-and-security-boundary.md) |
| Native Path 与流式语义 | [native-path-and-streaming.md](native-path-and-streaming.md) |
| Generation envelope 与状态 | [generation-state.md](generation-state.md) |
| Function tool 与私有扩展 | [tools-and-extensions.md](tools-and-extensions.md) |
| 普通参数与条件输出兼容 | [parameter-compatibility.md](parameter-compatibility.md) |
| 错误与客户端可见结果 | [errors-and-client-results.md](errors-and-client-results.md) |
| 功能验收要求与非目标 | [acceptance-and-non-goals.md](acceptance-and-non-goals.md) |

## 用户结果

受信用户把本地 Agent 或 OpenAI-compatible SDK 指向一个稳定 base URL，使用私有用户表中的 Bearer API
Key 与 Public Model 调用服务。客户端不需要、也不能选择上游 Provider、真实模型、URL、credential 或候选切换。

兼容目标包括：

1. OpenAI-compatible Chat Completions、Responses 与 Embeddings HTTP JSON/SSE；
2. 同协议 Native 媒体，以及按任务分离的 Chat Native audio understanding、ASR/TTS 与 voice conditioning；
3. 只转换显式共同语义的 Chat/Responses Bridge；
4. MCP `2026-07-28` stateless 与 legacy initialize/session lifecycle；
5. 只有在单独声明的 contract 内，才包含 Codex、Hermes 等客户端特有 profile、transport 与 tool loop。

“请求可转发”不等于“某个 Agent 完整兼容”。兼容承诺必须由本文档明确 endpoint、stream、tool、state 与
Provider 边界，并由实施现状记录对应证据。

## 关联文档

- [产品范围](../product-scope/README.md)
- [Public Model 与模型能力契约](../model-capability/README.md)
- [配置与凭证](../configuration-credentials/README.md)
- [路由与 Provider 韧性](../routing-resilience/README.md)
- [运行期观测与 OpenTelemetry](../observability/README.md)
