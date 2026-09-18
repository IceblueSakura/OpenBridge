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

大合同域按所有权拆分为同名目录下的叶子文档；父页只保留导言与叶子导航，合同正文与验收编号归叶子唯一拥有。
各域验收编号归属：`API-01..19`（gateway-api/）、`MODEL-01..16`（model-capability/）、`EMB/IMG/FILE/AUD/IMG-GEN`（extended-capabilities/）、
`CFG-01..19` + `OAUTH-01..12` + `GROK-01..09`（configuration/）、`OBS-01..08`（observability）。

Chat/Responses Generation 的目标语义管线（双向 decode → 富语义 IR → encode，IR 为唯一语义权威）是已接受、尚未完成的
设计目标：决策依据由 [ADR-0001](../decisions/0001-generation-ir-authority.md) 拥有，阶段计划由
[下一步目标](../implementation-plans/next-goal.md)拥有，目标行为见[Native Path 与流式语义](gateway-api/native-and-streaming.md)，实现差距见[当前状态边界](../implementation-status/current-boundaries.md)。本目录内未标注"设计目标"的条款均为当前有效行为合同。

需求中的"必须""不得""只允许"是验收约束，不表示当前实现已经满足。行为变更必须先明确用户结果、失败语义、
安全/资源边界与非目标，再由[当前开发焦点](../implementation-plans/current-focus.md)记录用户已授权的可观察切片；文档本身不授予实施权限。
