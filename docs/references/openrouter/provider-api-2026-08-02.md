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
- 官方 streaming 示例把终态放在 data JSON 的 `type: "response.done"`，并在嵌套 `response.status` 标记
  `completed`；其后还有 `[DONE]`。OpenBridge 使用 `response.done` 的 status 判定语义终态。
- 当前注册使用基础 model id `nvidia/nemotron-3-ultra-550b-a55b`，不使用 `:free` 变体。
- `HTTP-Referer`、`X-Title` 等可选 attribution/routing header 不属于当前所需认证事实，OpenBridge 不从下游
  转发这些字段。

## 未采用与适用边界

- OpenRouter 文档描述的是平台统一 Responses surface，不自动证明 Nemotron 3 Ultra 的每个模型级参数组合。
  当前注册只开放 canonical 模型已声明的 reasoning level 与 function tool，并保守关闭 parallel tools、image 和
  structured output；真实 wire 兼容性仍需单独验收。
- `:free` 模型页声明其免费 endpoint 会记录会话，且不应发送机密或个人信息。本次采用基础 model id，避免把
  该额外数据政策隐式带入默认路由；这不证明基础模型的商业、隐私或保留条款。
- 静态文档与确定性测试不证明当前账号权限、配额、在线模型可用性、真实 tool/reasoning wire 行为或服务质量。
  上线前必须在获得明确授权后执行脱敏的真实 Provider 验收。

## 复核条件

OpenRouter endpoint、认证方式、模型 id、具体模型能力或数据政策发生变化，或计划启用有状态 Responses、
`:free` 变体、Provider routing、attribution header 时，必须重新检查官方文档与模型页，并更新注册表契约和测试。
