# 2026-08-27 OpenRouter GLM-5.3-Flash 接入验证

## 来源声明

- OpenRouter model endpoint：<https://openrouter.ai/api/v1/models/z-ai/glm-5.3-flash/endpoints>。
- 精确上游 model ID：`z-ai/glm-5.3-flash`；下游 Public Model：`glm-5.3-flash`。
- OpenRouter endpoint 记录声明 text/image/video input、text output、1,048,576 context，以及 reasoning、tools、tool choice、response format 等参数；协议级字段仍需按实际 Chat/Responses wire 独立验证。

## 已执行验证与差异

- 时间：2026-08-27，Asia/Shanghai；基线 commit 为 `022b5cd1089ba627a7a82dd47049949ff91dd42d`，GLM 接入代码与本文属于同一待提交变更。
- 路径：本地编译的 OpenBridge loopback 实例，经现有 OpenRouter credential pool 访问真实 Z.AI endpoint；全部使用受控合成文本、1×1 PNG data URL 和合成 function schema。
- Chat Completions 与 Responses 的 non-streaming、SSE streaming、terminal 与 usage 均成功；Chat/Responses PNG data URL image 均成功。
- `tool_choice=auto` 的 non-streaming 与 streaming function call 在 Chat/Responses 均返回 `lookup`；`parallel_tool_calls=true` 被接受；named tool choice 被上游以 `Tool choice must be auto` 拒绝。两种协议的 `strict: true` function schema 都要求唯一字符串字段 `value`，实际 arguments 均精确返回 `{"value":"alpha"}`。
- Chat `json_object` 多次生成可解析且符合预期键集合的 JSON。Chat JSON Schema 请求被静默忽略并返回普通文本；Responses `json_object` 一次合法、一次夹带额外文本，因此两者不能提升为可靠的 Responses structured-output 保证。
- Hermes `obc` 与 `obr` 各以 `glm-5.3-flash` 完成一次无工具 turn，均为单次模型调用、无 retry/fallback；Hermes catalog match 使用 1,048,576 context。
- 原始 credential、认证 header、请求/响应正文、Provider request ID 与真实用户内容均未保存到仓库。

## 当前代码结果

- canonical model 使用 `z-ai/glm-5.3-flash`，Public Model 使用 `glm-5.3-flash`，通过既有 OpenRouter pool 提供 Chat/Responses 双 Native route。
- 两种协议都公开 image 与 Auto-only function tools；保留已接受且输出符合 schema 的 strict function schema，以及 parallel 请求开关。
- Chat structured output 收窄为 `JsonObject`；Responses structured output 不公开；named/required/none tool choice 不公开。
- Z.AI 直连 Provider 不在本次范围，当前路由仍是 OpenRouter 聚合入口。

## 证据边界

本记录只证明当时账号、网络、OpenRouter 单一 Z.AI endpoint、最小合成 payload 和短输出。它不证明 remote image/JPEG、video、多图和大小上限、接近 1M 的长上下文、两个并行 tool call 的实际同时生成、复杂 tool loop、质量、价格、配额、其他账号/区域、负载、长期稳定性或未来 OpenRouter 行为。扩大能力前必须重新读取当前来源并执行对应真实请求。
