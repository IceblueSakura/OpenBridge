# Zhipu AI China / Z.AI API 协议入口

- Last reverified：2026-08-31；刷新官方 OpenAI-compatible Chat、GLM-5.3 Responses 与 structured output 来源，并对已配置中国 endpoint 执行有界 JSON/SSE 探测。
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

2026-08-31 对已配置 `glm-5.3` 执行 16-token 上限的 Chat/Responses × JSON/SSE probe，四种组合均返回 200；Responses JSON 以 completed response 结束，SSE 产生 typed events 并以 `response.completed` 结束。该 probe 不证明 structured output、reasoning 参数、function tool、state、媒体、外部 SDK/Agent、负载、长期运行、其他账户/地域或未来可达性。

OpenBridge 当前映射见[Model 与 Provider 映射](../../implementation-status/model-provider-mapping.md)。
