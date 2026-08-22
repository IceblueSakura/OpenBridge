# 功能需求

本目录只定义当前有效的产品行为、失败语义、安全边界与非目标，不记录代码完成度、测试运行结果、实施时间线或
候选设计。实现与验证事实统一由[实施现状](../implementation-status/README.md)拥有。

| 功能域 | 唯一入口 | 回答的问题 |
|---|---|---|
| 产品范围 | [产品范围](product-scope/README.md) | 服务解决什么问题、当前接口与明确非目标 |
| 网关 API | [网关 API 域](gateway-api/README.md) | endpoint、认证、JSON/SSE、MCP、tool、state 与错误如何表现 |
| 运行期观测 | [观测域](observability/README.md) | request/attempt、OTLP signals 与本地 HTTP snapshot 如何安全产生 |
| 模型与能力 | [模型能力域](model-capability/README.md) | Public Model 如何公开固定契约并执行 zero-egress 预检 |
| 配置与凭证 | [配置凭证域](configuration-credentials/README.md) | Bootstrap、registry、API-key/OAuth 与 egress 如何受信管理 |
| 路由与韧性 | [路由韧性域](routing-resilience/README.md) | `NativeFirst`/`SourceFirst`、retry/fallback、rotation 与 cooldown |
| 扩展能力 | [扩展能力域](extended-capabilities/README.md) | Embeddings 与 Native image/file/audio 如何分层、编译与预检 |

扩展能力的具体 contract 分别见 [Embeddings](extended-capabilities/embeddings.md)、
[图片](extended-capabilities/native-image.md)、[文件](extended-capabilities/native-file.md)、
[音频](extended-capabilities/native-audio.md)与 [Images 生成](extended-capabilities/native-image-generation.md)。
ChatGPT subscription credential 见 [OAuth lifecycle](configuration-credentials/upstream-oauth-credential-lifecycle.md)。

ChatGPT 当前边界必须使用三个不同数量描述：五个固定 Responses-native Target；其中四个 Public Model 只有
ChatGPT source；第五个 Target 属于还包含 OpenAI source 的 `gpt-5.6-sol` Public Model。不得再用“四个 target”或
“五个 ChatGPT-only Public Model”混写。

功能需求中的“必须”“不得”“只允许”是验收约束，不代表当前实现已经满足。需求变更先在对应唯一 owner 中
明确用户结果、失败与安全边界；具体实施只由 `../implementation-plans/current-focus.md` 管理，且不能反向扩大需求。
