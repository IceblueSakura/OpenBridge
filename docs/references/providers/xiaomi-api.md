# Xiaomi MiMo API 协议入口

- Last reverified：2026-08-31；刷新 Chat、Responses 与 Models 官方页面。
- Recheck trigger：origin、认证、Chat/Responses/Models endpoint 或媒体协议变化。

## 来源与范围

- [Chat Completions API](https://mimo.mi.com/docs/zh-CN/api/chat/openai-api)
- [Responses API](https://mimo.mi.com/docs/zh-CN/api/chat/responses)
- [Models API](https://mimo.mi.com/docs/zh-CN/api/model/list-models)
- [图片协议与固定 wire 观察](xiaomi-image.md)
- [音频协议与固定 wire 观察](xiaomi-audio.md)

本文只记录公共 origin、认证和 endpoint，不复制逐模型能力、参数或下线列表。

## 入口与认证

- API origin 为 `https://api.xiaomimimo.com`。
- Chat Completions、Responses 和 Models 相对入口分别为 `/v1/chat/completions`、`/v1/responses` 和 `/v1/models`。
- 认证支持 `api-key: ***` 或 `Authorization: Bearer ***`，二选一。
- 官方 Responses 文档将 `background` 与 `previous_response_id` 列为不支持。
- [Chat 结构化输出专页](https://mimo.mi.com/docs/en-US/quick-start/usage-guide/text-generation/structured-output)明确为 MiMo-V2.5/Pro 提供 `json_object`；Chat API reference 同时只列 `text`，两者存在官方文档冲突，因此 executable caps 只保留专页明确的 Chat JSON Object，不提升 JSON Schema。
- Responses 当前只列 `text` format，也未声明 `prompt_cache_key` 或 `include`；这些 Responses 字段不作为 executable caps 公开。

Models 目录可见性不证明某个 operation、参数、streaming 或媒体任务当前可用。具体模型能力和生命周期应直接读取 MiMo 官方文档；OpenBridge 旧运行时映射见[固定归档](https://github.com/IceblueSakura/OpenBridge/blob/4f13ecefa21265a6ec5aa967278e81f03039f585/docs/implementation-status/model-provider-mapping.md)，不是 v2 当前能力。

## Wire 观察（2026-09-28 实测，`mimo-v2.6-pro`）

- Responses 响应体**不回显任何 reported settings**（无 `tools`/`tool_choice`/`parallel_tool_calls`/`instructions` 等），usage 亦不含 `cache_write_tokens`；固定 SDK 对这些字段为 required 非空/整数，导致 SDK 严格解析失败（[证据](../../implementation-status/evidence/2026-09-28-deepseek-xiaomi-provider-live-matrix.md)）。
- 响应体含非标准 `output_text` 便捷字段；Chat thinking 经 `reasoning_content` 返回（官方建议续轮回传），Chat usage 细分为 `cached_tokens`/`reasoning_tokens`。
- 官方文档声明 `tool_choice` 非 `auto` 会被后端**静默移除**；`finish_reason` 枚举含非标准的 `repetition_truncation`（另有 `content_filter`）。流式 chunk 的 `created` 逐帧递增，`id`/`model` 恒定，续传帧用 `null` 复用已声明的 call 身份。

## 证据边界

本文不替代真实账号、错误、负载或长期运行验证。动态 endpoint 和 Provider 行为变化时需要重新读取官方资料。
