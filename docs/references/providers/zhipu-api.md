# Zhipu AI China / Z.AI API 协议入口

- Last reverified：2026-09-29 UTC；重读 GLM-5.3 官方页面并核对当前目录与选定 Chat wire。2026-08-31 的其他来源/协议证据仍按其原范围解释。
- Recheck trigger：`/api/paas/v4` 或 `/api/v1` 路径、认证、Responses 模型范围、SSE 终态、structured output 或工具合同变化。

## 来源与范围

- [OpenAI SDK 兼容调用](https://docs.bigmodel.cn/cn/guide/develop/openai/introduction)
- [GLM-5.3 模型页](https://docs.bigmodel.cn/cn/guide/models/text/glm-5.3)
- [GLM-5.2 模型页](https://docs.bigmodel.cn/cn/guide/models/text/glm-5.2)
- [GLM-5.3-Flash 模型页](https://docs.bigmodel.cn/cn/guide/models/vlm/glm-5.3-flash)
- [结构化输出](https://docs.bigmodel.cn/cn/guide/capabilities/struct-output)

本文只记录协议入口与采用边界，不复制逐模型能力矩阵、context、参数或价格。

## 协议边界

- OpenAI-compatible Chat 使用 `https://open.bigmodel.cn/api/paas/v4` 下的 `/chat/completions`。
- GLM-5.3 官方模型页另列 OpenAI Responses 协议；其固定入口位于同一受信 origin 的 `/api/v1/responses`，不能把旧 `/api/paas/v4/responses` 的 404 外推为该协议不存在。
- 官方 structured output 指南使用 Chat `response_format: {"type":"json_object"}`，并要求 prompt 明确要求 JSON；它不是 JSON Schema 保证。
- 模型页未明确列出的 Responses 模型、参数和工具能力保持未知，不能从 GLM-5.3 外推。

## 执行边界

本轮只准入 GLM-5.3 的 Chat 绑定；`request_id` 等具名映射见 [API-key profiles](../../architecture-v2/api-key-text-profiles.md)，v2 实际入口与 pi 验收见 [onboarding evidence](../../implementation-status/evidence/2026-09-29-api-key-provider-onboarding.md)。官方页还提示有过 Coding Plan 订阅的账号可能仅能用 Chat；未直接将历史 Responses 成功移植为当前 native entry。

2026-08-31 对已配置 `glm-5.3` 执行 16-token 上限的 Chat/Responses × JSON/SSE probe，四种组合均返回 200；Responses JSON 以 completed response 结束，SSE 产生 typed events 并以 `response.completed` 结束。该 probe 不证明 structured output、reasoning 参数、function tool、state、媒体、外部 SDK/Agent、负载、长期运行、其他账户/地域或未来可达性。

OpenBridge 旧运行时映射见[固定归档](https://github.com/IceblueSakura/OpenBridge/blob/4f13ecefa21265a6ec5aa967278e81f03039f585/docs/implementation-status/model-provider-mapping.md)，不是 v2 当前能力。
