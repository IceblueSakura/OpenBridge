# Kimi CN Provider 状态

## 当前实现

- Provider family 为 `kimi-cn`，固定 origin 为 `https://api.moonshot.cn`，使用 `kimi-primary` API-key pool。
- `kimi-k3` 只注册 Chat Native；Public Model 在缺少 Responses Native 时补充 Responses-via-Chat Bridge。
- Chat/Responses 公开文本、streaming、`PlainText` reasoning 和 `none/low/high/max`。
- Kimi Chat API 对 `temperature`、`top_p`、`presence_penalty`、`frequency_penalty` 使用闭合 ignore 规则；Chat 可保留
  `seed`，Responses Bridge 不公开无法转换的 `seed`。
- `n`、`logprobs`、`top_logprobs` 改变输出数量/结构，两个下游 interface 均不公开，并在 Provider egress 前 fail closed。

## 所有权与确定性证据

- 注册与 wire 规则：[`src/providers/kimi_cn/`](../../../src/providers/kimi_cn/)。
- `tests/provider_contract.rs`、`tests/provider_boundary_contract.rs` 保护 Chat-only surface、模型替换、API-key 与相对 URI。
- `tests/forwarding_contract.rs` 保护 Native/Bridge 参数处置、unknown parameter 与 zero-egress 错误。

## 真实 Provider 证据

[2026-08-09 文字矩阵](../evidence/real-provider/2026-08-09-text-generation-none-high-matrix.md)记录 `kimi-k3`
Chat/Responses × JSON/SSE × `none/high` 正常首选路径，成功单元均有完整终态；`none` 无可观察 reasoning。

同日定向参数请求确认非默认 `temperature` 在闭合 ignore 策略下仍成功，`n/logprobs/top_logprobs` 在本地以
`unsupported_model_capability` 拒绝，未知字段以 `unknown_parameter` 拒绝；这些结果没有保存请求/响应正文。

## 未证明边界

其他 Moonshot endpoint、原生 Responses、更多参数组合、账号权限、外部 SDK/Agent、负载和长期运行未证明。Kimi K3 的 reasoning
关闭结论只适用于记录中的部署、账号和时间点。

## 相关文档

- [Kimi API 参考](../../references/providers/kimi/api.md)
- [Kimi Models 参考](../../references/providers/kimi/models.md)
