# Xiaomi MiMo API 协议入口调研（复核于 2026-08-09）

## 来源与范围

本文只记录 Xiaomi MiMo 的公共 API origin、Chat/Responses 入口与认证事实。模型目录见 [models.md](models.md)；图片与音频协议按功能拆分：

- [模型目录与变更](models.md)
- [图片理解协议与真实观察](image.md)
- [全模型语音能力与调用途径](audio.md)

这些页面是外部 Provider 快照，不替代 OpenBridge 当前实现状态或功能需求。

- [Chat Completions API](https://mimo.mi.com/docs/zh-CN/api/chat/openai-api)
- [Responses API](https://mimo.mi.com/docs/zh-CN/api/chat/responses)
- [结构化输出](https://mimo.mi.com/docs/zh-CN/quick-start/usage-guide/text-generation/structured-output)
- [Models list](https://mimo.mi.com/docs/zh-CN/api/model/list-models)
- [模型下线说明](https://mimo.mi.com/docs/zh-CN/updates/deprecate)

## 观察事实

- API origin 为 `https://api.xiaomimimo.com`；Chat Completions 请求地址为 `https://api.xiaomimimo.com/v1/chat/completions`，Responses 请求地址为 `https://api.xiaomimimo.com/v1/responses`，模型列表为 `https://api.xiaomimimo.com/v1/models`。
- 认证支持两种方式（二选一，加入请求头）：
  - `api-key: $MIMO_API_KEY`
  - `Authorization: Bearer $MIMO_API_KEY`
- Responses 文档明确不支持 `background` 与 `previous_response_id`。
- 独立 structured-output 页面明确列出 `mimo-v2.5` 与 `mimo-v2.5-pro`，使用
  `response_format: {"type":"json_object"}`；prompt 必须明确只返回 JSON，并完整描述字段、层级和类型。流式响应需要拼接完整文本后
  再解析。
- 官方把该能力定义为 JSON mode：只保证输出是合法 JSON，不保证字段和类型符合预设 JSON Schema，并建议客户端使用
  `jsonschema` 库另行校验。因此不能把外部校验示例外推成上游 `json_schema` mode。
- Responses API 的 `text.format` 同样列出 JSON object 形态，且可用模型为 `mimo-v2.5` 与 `mimo-v2.5-pro`。页面明确只有已声明参数会
  正常处理，未定义参数可能被过滤或报错；这些文字能力不能外推到专用音频 task。
- Chat 与 Responses 的 `tool_choice` 都只列出 `auto`。官方明确说明任何非 `auto` 值都会被后端移除，模型行为仍等同于 auto；因此
  某次 required/named 请求产生 tool call 不能证明该 choice 生效。
- 两种协议都声明 function tool 的 `strict` 字段及受限 JSON Schema 遵循；当前请求参数列表没有声明 `parallel_tool_calls`。自然产生
  多个 tool call 不等于客户端可以控制并行选择。
- Chat 使用 `thinking.type` 控制 reasoning：`enabled` 开启、`disabled` 关闭；官方关闭示例的
  `completion_tokens_details.reasoning_tokens` 为 0。
- Responses 使用标准 `reasoning.effort`，接受 `none`、`low`、`medium`、`high`。`none` 关闭 reasoning；官方明确说明
  `low`、`medium`、`high` 当前都只是开启 reasoning，行为完全相同，尚不支持细粒度强度差异。
- 旧 `mimo-v2-pro`、`mimo-v2-omni`、`mimo-v2-flash` 与 `mimo-v2-tts` 已于 2026-06-30 下线；新接入必须使用当前 model ID（见 [models.md](models.md)）。

## 证据边界

endpoint、认证、JSON mode 与上述 reasoning 参数只证明官方协议声明，不能替代逐 model/operation 的真实账号、streaming、Bridge、负载或长期运行验证。
MiMo 接受三个开启值不表示它们当前产生不同推理强度。动态模型目录和 Provider 行为会变化；使用前须按功能页的日期与证据层重新复核。
