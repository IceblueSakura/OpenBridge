# OpenRouter Provider API 快照（2026-08-02）

## 来源与检查范围

- [Chat Completions API](https://openrouter.ai/docs/api/api-reference/chat/send-chat-completion-request?explorer=true)
- [Models API](https://openrouter.ai/docs/api/api-reference/models/get-models)
- [Responses API Beta](https://openrouter.ai/docs/api/reference/responses/overview)
- [Nemotron 3 Ultra Free model page](https://openrouter.ai/nvidia/nemotron-3-ultra-550b-a55b%3Afree/api)

本文区分官方页面描述与一次固定日期的 live wire 观察；二者都只适用于 OpenRouter。

## 1. 官方 API 事实

- API base 为 `https://openrouter.ai/api/v1`。
- Chat Completions、Responses 和 Models 相对 path 分别为 `/chat/completions`、`/responses`、`/models`。
- API key 使用 `Authorization: Bearer <OPENROUTER_API_KEY>`。
- Responses 页面描述 JSON/SSE、reasoning 和 function tool；该 surface 是无状态的，`store: true` 和非空 `previous_response_id` 返回 400。
- `HTTP-Referer`、`X-Title` 是可选 attribution/routing header，不是 Bearer 认证本身。

## 2. 官方示例与 live wire 差异

官方 streaming 示例曾显示顶层 `type: "response.done"`、嵌套 `response.status: "completed"`，随后发送 `[DONE]`。

2026-08-02 对基础模型 `nvidia/nemotron-3-ultra-550b-a55b` 的两次成功 Responses streaming 观察均得到：

- HTTP 200；
- data-only SSE，没有 `event:` line；
- terminal data JSON 顶层 `type: "response.completed"`；
- 嵌套 `response.status: "completed"`；
- terminal 后另有 `[DONE]`；
- 没有出现 `response.done`。

该观察只记录原始 upstream wire 差异。它不证明错误 terminal、其他模型、全部参数或未来版本使用同一事件。

## 3. 模型与数据政策边界

- 平台统一 Responses surface 不自动证明每个模型支持所有参数组合。
- 基础模型与 `:free` 变体有不同目录元数据和供应条件。
- `:free` 模型页声明免费 endpoint 会记录会话，不应发送机密或个人信息；该政策不能自动外推到基础模型或其他 endpoint。
- 一次成功请求只证明该模型、账户和时间点的成功流，不证明长期权限、配额、SLA、tool/reasoning 细节或所有错误行为。

## 4. 复核条件

endpoint、认证、模型 id、Responses beta 行为、数据政策、attribution header 或具体模型页面变化时，需要重新采集官方资料和 wire transcript，并分别记录模型变体。
