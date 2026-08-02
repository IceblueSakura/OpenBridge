# DeepSeek 与 Xiaomi MiMo 协议入口

## 证据范围

检查日期：2026-08-02。本文只记录两家官方文档公开的协议入口与认证方式，不证明 OpenBridge 已接入 target、
Route、真实 credential 或真实 Provider。

## DeepSeek

官方 [首次调用说明](https://api-docs.deepseek.com/guides/function_calling/) 将 OpenAI 格式 base URL 记为
`https://api.deepseek.com`，示例通过 Bearer API key 调用 Chat Completions。官方
[Chat Completions API](https://api-docs.deepseek.com/api/create-chat-completion) 的相对入口为
`/chat/completions`。

官方[更新记录](https://api-docs.deepseek.com/updates/)与
[模型价格页](https://api-docs.deepseek.com/quick_start/pricing/)声明 V4 的当前模型名为
`deepseek-v4-pro` 与 `deepseek-v4-flash`；旧 `deepseek-chat`/`deepseek-reasoner` 名称已进入停用边界，
当前注册不使用旧别名。

本轮只据此声明 OpenAI-compatible Chat Completions；不声明 Responses。能力上界只保留 streaming 与
function calling，其他能力在没有本轮接入证据时保持关闭。

## Xiaomi MiMo

官方 [Chat Completions API](https://mimo.mi.com/docs/zh-CN/api/chat/openai-api) 的请求地址为
`https://api.xiaomimimo.com/v1/chat/completions`。官方
[Responses API](https://mimo.mi.com/docs/zh-CN/api/chat/responses) 的请求地址为
`https://api.xiaomimimo.com/v1/responses`，并明确不支持 `background` 与 `previous_response_id`。
两份文档都允许 `api-key` 或 `Authorization: Bearer`；本轮静态 Provider 定义复用 OpenBridge 现有 Bearer
credential 机制，不新增第二种认证 adapter。

官方[模型列表](https://mimo.mi.com/docs/zh-CN/api/model/list-models)列出 `mimo-v2.5-pro` 与 `mimo-v2.5`；
当前文本 Provider 注册只使用这两个模型，不纳入 ASR/TTS 变体。

MiMo 的能力上界只保留两种协议的 streaming 与 function calling；图像、structured output 等字段即使出现在
官方文档中，也不在本轮静态接入范围内扩大。

## OpenBridge 适用边界

- `ProviderKind`、静态 contract、相对 path、固定 endpoint、credential locator 与上述四个文本模型可以据此加入代码。
- DeepSeek 只注册 Chat Upstream API；下游 Responses 能力来自 OpenBridge 的显式 Protocol Bridge，不属于
  DeepSeek 原生能力声明。
- MiMo 注册 Chat/Responses Native Upstream API，但仍保持 `background`、`previous_response_id` 等未验证能力关闭。
- 静态单元测试只证明 adapter 选择与请求改写，不证明真实 Provider 接受请求或完整 SSE/tool lifecycle 兼容。
- 实际兼容结论仍需执行独立协议测试和真实 Provider 验证。
