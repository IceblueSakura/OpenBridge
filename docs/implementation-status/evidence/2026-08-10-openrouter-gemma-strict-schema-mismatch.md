# 2026-08-10 OpenRouter Gemma strict schema 差异

## 来源声明

- OpenRouter 公开入口：[`GET /api/v1/models`](https://openrouter.ai/api/v1/models) 与 [model endpoint 列表](https://openrouter.ai/docs/api/api-reference/endpoints/list-endpoints)。
- 精确上游 model ID：`google/gemma-4-31b-it:free`。
- 当时目录/endpoint 信息将该模型纳入 structured-output capability surface；仓库没有保留原始 Models payload，因此无法在当前 checkout 重放当时的完整字段集合。

## 已执行测试与差异

- 时间：2026-08-10，Asia/Shanghai。
- 路径：真实 OpenRouter free endpoint；使用当时配置的账号、网络和固定请求。
- 请求边界：strict JSON Schema 输出请求；原始请求 body、schema 和响应 body 未入库，只保留测试形状。
- 观察：请求成功返回内容，但 JSON 被 Markdown code fence 包裹，未可靠遵循 strict schema。
- 差异：目录/endpoint 的 structured-output 可见性不能提升为 strict JSON Schema 保证；本次结果只支持保守的 `json_object`。

## 当前代码结果

该差异促成 `src/providers/openrouter/registration.rs` 对 Gemma Target 关闭 function-tool strict schema，并把 structured output 收窄为 `JsonObject`。模型能力本身继续由代码和运行中的扩展 Models API 自描述，本文不复制其他模型元信息。

## 证据边界

本记录只证明当时 exact free model、账号、网络和请求形状下的差异。它不证明其他 schema、Provider endpoint、基础非 free 变体、账户、区域、SDK、负载、长期可用性或当前 OpenRouter 行为。由于原始 payload 未保留，不能据此做字节级回归；重新扩大 strict schema 能力前必须重新读取当前 official/OpenRouter 来源并执行独立请求。
