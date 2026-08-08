# Alibaba Cloud Model Studio Provider 状态

## 当前注册

- Provider family：`bailian`；
- 可信 Base URL：`https://dashscope.aliyuncs.com/compatible-mode/v1`；
- credential pool：`bailian-primary`，仅允许 API key；
- 固定注册 12 个 Target，覆盖 GLM、Qwen generation/image/audio/embedding 和 DeepSeek fallback；
- 上游接口为 OpenAI-compatible Chat Completions 或 Embeddings；没有 Responses Native，缺失的 Responses coverage
  由 Public Model 编译器在可表达范围内补充 Responses-via-Chat Route；
- Provider Chat ceiling 允许 `PlainText` reasoning，但只有已真实确认的 `glm-5.2`、`qwen3.7-max`、
  `qwen3.7-plus` Target 保留该事实；其他 Bailian Chat Target 显式收窄为 `Unknown`。

## 真实验证

2026-08-08 使用当前私有 credential 和真实下游用户 key 验证：

- `glm-5.2` 的 Chat/Responses × JSON/SSE × reasoning 字段省略/high 共 8 个单元全部成功；
- `qwen3.7-max` 与 `qwen3.7-plus` 的 Chat/Responses 字段省略 JSON/SSE 全部成功，两个模型未声明 high
  level，因此 high 请求继续在本地能力预检阶段拒绝；
- 三个模型省略 reasoning 字段时会返回非空明文 `reasoning_content`，Responses Bridge 可将其保留为 reasoning item；
- 三个模型的 Chat SSE 会在 `finish_reason` 后发送 usage-only chunk；当前 Bridge 接受一个严格
  `choices: []` + `usage` object 块，随后等待 `[DONE]` 并产生唯一 `response.completed`；
- 标准 `reasoning_effort: "none"` 与 Hermes custom off wire shape
  （`reasoning_effort: "none"` + `think: false`）均让三个模型的 Chat JSON/SSE reasoning 内容为空。

最终矩阵和剩余错误边界见 [`real-e2e-test-2026-08-08.md`](../real-e2e-test-2026-08-08.md)。

## 证据边界

`tests/example_config.rs` 与 `tests/provider_contract.rs` 验证 Provider ceiling、目标级 reasoning 收窄、固定 endpoint、
credential kind 和 Route 编译；`tests/bridge_conversion_contract.rs` 与 `tests/bridge_forwarding_contract.rs`
验证 usage-only SSE lifecycle 和 Responses terminal。

这些证据不把三个已实测模型的 reasoning 事实外推到其他 Bailian Target，也不证明其他账号、区域、未来 Provider 行为、外部
SDK、负载或长期运行兼容性。
