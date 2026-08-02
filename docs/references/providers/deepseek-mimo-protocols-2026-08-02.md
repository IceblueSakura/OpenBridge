# DeepSeek 与 Xiaomi MiMo 协议入口

## 证据范围

检查日期：2026-08-02。本文只记录两家官方文档公开的协议入口与认证方式，不证明 OpenBridge 已接入 target、
Route、真实 credential 或真实 Provider。

## DeepSeek

官方 [首次调用说明](https://api-docs.deepseek.com/guides/function_calling/) 将 OpenAI 格式 base URL 记为
`https://api.deepseek.com`，示例通过 Bearer API key 调用 Chat Completions。官方
[Chat Completions API](https://api-docs.deepseek.com/api/create-chat-completion) 的相对入口为
`/chat/completions`。

本轮只据此声明 OpenAI-compatible Chat Completions；不声明 Responses。能力上界只保留 streaming 与
function calling，其他能力在没有本轮接入证据时保持关闭。

## Xiaomi MiMo

官方 [Chat Completions API](https://mimo.mi.com/docs/zh-CN/api/chat/openai-api) 的请求地址为
`https://api.xiaomimimo.com/v1/chat/completions`。官方
[Responses API](https://mimo.mi.com/docs/zh-CN/api/chat/responses) 的请求地址为
`https://api.xiaomimimo.com/v1/responses`，并明确不支持 `background` 与 `previous_response_id`。
两份文档都允许 `api-key` 或 `Authorization: Bearer`；本轮静态 Provider 定义复用 OpenBridge 现有 Bearer
credential 机制，不新增第二种认证 adapter。

MiMo 的能力上界只保留两种协议的 streaming 与 function calling；图像、structured output 等字段即使出现在
官方文档中，也不在本轮静态接入范围内扩大。

## OpenBridge 适用边界

- `ProviderKind`、静态 contract 与相对 path 可以据此加入代码。
- 本轮不注册 endpoint base、credential locator、Model、Upstream Target、Route 或 Public Model。
- 静态单元测试只证明 adapter 选择与请求改写，不证明真实 Provider 接受请求或完整 SSE/tool lifecycle 兼容。
- 真正接入链路前必须重新复核官方文档，并执行独立协议测试和真实 Provider 验证。
