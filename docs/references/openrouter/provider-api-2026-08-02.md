# OpenRouter Provider API 快照（2026-08-02）

## 来源与检查范围

本快照只记录 OpenBridge 当前 OpenRouter Native 注册所需的官方事实，不把 OpenRouter 的全部统一 API
能力外推为具体模型能力。检查来源：

- [Chat Completions API](https://openrouter.ai/docs/api/api-reference/chat/send-chat-completion-request?explorer=true)
- [Models API](https://openrouter.ai/docs/api/api-reference/models/get-models)
- [Responses API Beta](https://openrouter.ai/docs/api/reference/responses/overview)
- [Nemotron 3 Ultra Free 模型页](https://openrouter.ai/nvidia/nemotron-3-ultra-550b-a55b%3Afree/api)

## 已采用事实

- OpenRouter API base 是 `https://openrouter.ai/api/v1`，Chat Completions 相对 path 是
  `/chat/completions`，Responses 相对 path 是 `/responses`，Models 相对 path 是 `/models`。
- API key 通过 `Authorization: Bearer <OPENROUTER_API_KEY>` 提交。
- Responses API 支持 JSON 与 SSE、reasoning 和 function tool，但只提供无状态调用；`store: true` 和非空
  `previous_response_id` 会返回 400。OpenBridge 还保守关闭未在当前承诺内的 `background`。
- 官方 streaming 示例把终态写为 data JSON 的 `type: "response.done"`，并在嵌套 `response.status` 标记
  `completed`，其后还有 `[DONE]`；这项文档示例与下述真实 Nemotron-3 wire 不一致，OpenBridge 不再把它
  作为当前终态配置依据。
- 当前注册使用基础 model id `nvidia/nemotron-3-ultra-550b-a55b`，不使用 `:free` 变体。
- `HTTP-Referer`、`X-Title` 等可选 attribution/routing header 不属于当前所需认证事实，OpenBridge 不从下游
  转发这些字段。

## 未采用与适用边界

- 2026-08-02 使用基础模型 `nvidia/nemotron-3-ultra-550b-a55b` 先后执行修复前复现与修复后验收的真实
  Responses streaming 成功请求：上游均返回 HTTP 200 和 data-only SSE，所有 frame 均没有 `event:`；终态 data JSON 顶层
  `type=response.completed`、嵌套 `response.status=completed`，随后另发 `[DONE]`，未出现
  `response.done`。因此 OpenRouter 当前 adapter 与 LongCat 一样，从 data JSON 顶层 `type` 读取 OpenAI
  terminal 词汇；`[DONE]` 仍不是 Responses 语义终态。修复前网关在 EOF 将请求误记为失败，修复后相同
  Native route 的终态观测为 `outcome=completed`。
- OpenRouter 文档描述的是平台统一 Responses surface，不自动证明 Nemotron 3 Ultra 的每个模型级参数组合。
  当前注册只开放 canonical 模型已声明的 reasoning level 与 function tool，并保守关闭 parallel tools、image 和
  structured output；真实 wire 兼容性仍需单独验收。
- `:free` 模型页声明其免费 endpoint 会记录会话，且不应发送机密或个人信息。本次采用基础 model id，避免把
  该额外数据政策隐式带入默认路由；这不证明基础模型的商业、隐私或保留条款。
- 本次真实请求只证明该模型、该账号在该时刻的成功流；不证明失败 terminal、其他模型、所有参数组合、
  当前账号长期权限/配额、真实 tool/reasoning wire 行为、服务质量或未来 wire 稳定性。

## 复核条件

OpenRouter endpoint、认证方式、模型 id、具体模型能力或数据政策发生变化，或计划启用有状态 Responses、
`:free` 变体、Provider routing、attribution header 时，必须重新检查官方文档与模型页，并更新注册表契约和测试。
