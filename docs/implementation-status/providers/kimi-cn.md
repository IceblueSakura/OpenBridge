# Kimi CN Provider 状态

## 当前注册

- Provider family：`kimi-cn`；
- 可信 Base URL：`https://api.moonshot.cn`；
- credential pool：`kimi-primary`，仅允许 API key；
- Public Model：`kimi-k3`；
- 上游模型：`kimi-k3`；
- 上游接口：只注册 Chat Completions；
- Route：一个 Chat Native Route，adapter 相对路径为 `/v1/chat/completions`；Public Model 编译器在缺少 Responses Native 时自动
  补充一个 Responses-via-Chat Bridge Route；
- 当前公开契约：Chat/Responses 的文本、streaming 和 `PlainText` reasoning；模型 levels 为 `low`、`high`、`max`。
  没有 Responses Native、Embeddings 或动态 endpoint/credential，Bridge 仍受完整 preflight 的共同语义和能力边界约束。
- Kimi Chat Upstream API 通过类型化普通参数规则删除 `temperature`、`top_p`、`presence_penalty` 与
  `frequency_penalty`；Chat interface 还原样接受 `seed`，Responses Bridge 不公开无法转换的 `seed`。`n`、`logprobs` 与
  `top_logprobs` 会改变输出数量或结构，当前 API 将其显式禁用；Chat/Responses 固定 interface 均不公开，并在 egress 前返回带精确
  `param` 的 `unsupported_model_capability`。

## 证据边界

`tests/example_config.rs` 中的 `kimi_cn_k3_compiles_with_native_chat_and_auto_responses_bridge` 已验证 Provider、pool、endpoint、三层
模型身份、Target、Public Model、Chat Native/Responses Bridge Route、本地两协议规划以及 adapter 的相对请求路径和上游 model 替换。
`tests/provider_contract.rs` 同时验证 Kimi CN 使用 API-key、仅声明 Chat Native 上游基线，并保持相对 URI 与 credential header 的
Provider 边界。

2026-08-08 使用真实下游用户 key 和当前私有 Kimi credential 执行了 Chat/Responses × JSON/SSE × reasoning
字段省略/high 矩阵，8 个单元最终全部成功。Responses-via-Chat 的 JSON 能保留 reasoning item，两种 SSE reasoning
组合均包含 `response.completed`。标准 `reasoning_effort: "none"` 与 Hermes custom off wire shape
（`reasoning_effort: "none"` + `think: false`）的 Chat JSON/SSE 均为 HTTP 200 且 reasoning 内容为空。
完整边界见 [`real-e2e-test-2026-08-08.md`](../real-e2e-test-2026-08-08.md)。

2026-08-09 复核 Kimi 官方[模型参数参考](../../references/providers/kimi/models.md)，确认 K3 的 sampling/penalty 提示为固定值且建议
省略。当前严格策略下使用真实下游 key 对非默认 `temperature` 执行 Chat/Responses × JSON/SSE，4/4 返回 HTTP 200 与合法终态；
`n/logprobs/top_logprobs` 两协议 6/6 返回预期 `unsupported_model_capability`，未知字段两协议 2/2 返回
`unknown_parameter`。全部单元一次完成，没有最终 429/503、协议或传输错误；请求和响应正文均未保存。

确定性证据和本次真实请求不证明其他 Moonshot endpoint、账号权限、未来模型行为、外部 SDK、负载或长期运行兼容性；Kimi K3
可关闭 reasoning 的结论只限本次部署与时间点。
