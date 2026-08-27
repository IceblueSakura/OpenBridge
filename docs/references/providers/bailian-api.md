# 阿里云百炼 API 协议入口

- Last reverified：2026-08-27；刷新官方 Responses/GLM/DeepSeek/Qwen 来源，并以北京真实 endpoint 对三模型执行协议差分。
- Recheck trigger：地域域名、兼容协议、认证、原生媒体 API 或 hosted tool 变化。

## 来源与范围

- [OpenAI 兼容 Chat](https://help.aliyun.com/zh/model-studio/qwen-api-via-openai-chat-completions)
- [OpenAI 兼容 Responses](https://help.aliyun.com/zh/model-studio/qwen-api-via-openai-responses)
- [Anthropic 兼容 Messages](https://help.aliyun.com/zh/model-studio/anthropic-api-messages)
- [DashScope API 总览](https://help.aliyun.com/zh/model-studio/developer-reference/use-qwen-by-calling-api)
- [GLM 模型](https://help.aliyun.com/zh/model-studio/glm)
- [DeepSeek API](https://help.aliyun.com/zh/model-studio/deepseek-api)
- [DeepSeek V4 Flash](https://help.aliyun.com/zh/model-studio/deepseek-v4-flash)
- [Qwen3.8 Max](https://help.aliyun.com/zh/model-studio/qwen3-8-max)
- [图像生成与编辑](https://help.aliyun.com/zh/model-studio/qwen-image-generation-and-editing-api-reference)

本文只记录地域、认证和协议入口，不复制逐模型支持集合或能力参数。

## 入口与地域

OpenAI-compatible base URL 使用 `compatible-mode/v1`。北京、新加坡、美国、法兰克福和东京当前都提供业务空间专属域名；北京与新加坡的旧公共域名仍可使用，但官方建议迁移。模型、endpoint、API key 与 region 必须匹配，具体域名以官方地域文档为准。

Chat 与 Responses 相对入口分别为 `/chat/completions` 和 `/responses`。百炼同时提供 Anthropic-compatible Messages 与 DashScope 原生 API；原生 API 不是 OpenAI-compatible endpoint 的别名。

## 认证

请求使用 `Authorization: Bearer $DASHSCOPE_API_KEY`。API key 按地域隔离，凭证和业务空间 ID 不应写入文档或配置样例。

## 协议边界

- OpenAI-compatible、Anthropic-compatible 与 DashScope 原生协议拥有不同 request/response wire，不能仅凭模型名称互换。
- 图片、音频、视频、hosted tool 和 reasoning 的具体支持集合随模型、region 和协议变化，应直接查阅官方模型页。
- Models 目录或控制台可见性不证明账户 entitlement、参数组合、streaming 或长期可用性。

## 执行证据

2026-08-27 对 `glm-5.2`、`deepseek-v4-flash-0731` 与 `qwen3.8-max` 的真实北京 Responses 对比见[带日期证据记录](../../implementation-status/evidence/2026-08-27-bailian-responses-model-comparison.md)。该记录拥有实际 JSON/SSE、reasoning、structured output、function-tool continuation、state 与 Provider-wide/model-specific 归因；本文不复制动态模型级结果。

OpenBridge 当前映射见[Model 与 Provider 映射](../../implementation-status/model-provider-mapping.md)。
