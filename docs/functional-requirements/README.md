# 功能需求

本目录只定义当前有效的产品行为、失败语义、安全边界和非目标。实现完成度、未验证范围与实际证据分别由
[当前实现](../implementation-status/current-state.md)、[当前状态边界](../implementation-status/current-boundaries.md)和
[evidence](../implementation-status/evidence/README.md)拥有。

| 唯一 owner | 内容 |
|---|---|
| [产品范围](product-scope.md) | 产品目标、信任边界、接口范围与明确非目标 |
| [网关 API](gateway-api.md) | HTTP/MCP、认证、Generation、streaming、tool、state 与错误 |
| [模型与能力](model-capability.md) | Model/Public Model、Models API、启动校验与 zero-egress preflight |
| [配置与凭证](configuration-credentials.md) | Bootstrap、静态注册、API key、OAuth 与受信 egress |
| [路由与韧性](routing-resilience.md) | Route ordering、attempt、retry/fallback、rotation、cooldown 与取消 |
| [观测](observability.md) | request/attempt、OTLP signals 与本地 bounded HTTP snapshot |
| [扩展能力](extended-capabilities.md) | Embeddings、Native image/file/audio 与 Images Generations |

需求中的“必须”“不得”“只允许”是验收约束，不表示当前实现已经满足。行为变更必须先明确用户结果、失败语义、
安全/资源边界与非目标，再由[当前开发焦点](../implementation-plans/current-focus.md)授权一个可观察切片。
