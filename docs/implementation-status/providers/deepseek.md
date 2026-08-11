# DeepSeek Provider 状态

## 当前实现

- Provider family 为 `deepseek`，固定 origin 为 `https://api.deepseek.com`，使用 `deepseek-primary` API-key pool。
- `deepseek-v4-pro` 只注册 Chat Native，Public Model 在全局缺少 Responses Native 时补充 Responses-via-Chat Bridge。
- `deepseek-v4-flash` 注册 Chat/Responses Native，并与 Bailian Chat、OpenRouter 双协议 source 聚合；`SourceFirst` 保持
  DeepSeek 为两个下游协议的首选。
- Chat 只公开 `json_object`；Flash Responses 公开 `json_object` 与 strict JSON Schema。工具 strict 仍关闭。
- Pro/Flash 公共 reasoning 档位分别为 `none/high/max` 与 `none/low/high/max`；Chat 输出为 `PlainText`。

## 所有权与确定性证据

- 注册与 wire 规则：[`src/providers/deepseek/`](../../../src/providers/deepseek/)。
- `tests/provider_contract.rs`、`tests/provider_boundary_contract.rs` 保护 endpoint、API surface、认证和错误边界。
- `tests/forwarding_contract.rs` 与 Bridge tests 保护 DeepSeek Native/Bridge、JSON object、reasoning 和多 source 失败边界。

## 真实 Provider 证据

2026-08-11 直连请求确认 Models 列表包含 V4 Pro/Flash；Flash Chat `json_object` 成功、Chat `json_schema`
被 400 拒绝；Flash Responses `json_object` 和 `json_schema strict:true` 均成功且 strict 请求输出符合 schema。`/beta`
前缀的 generation 行为与根路径一致，但 `/beta/models` 为 404。

[2026-08-09 文字矩阵](../evidence/real-provider/2026-08-09-text-generation-none-high-matrix.md)记录 Pro/Flash
正常首选路径的 `none/high` Chat/Responses JSON/SSE 结果；它没有强制 Bailian 或 OpenRouter 后备。

## 未证明边界

Structured-output 探测只覆盖 Flash；Pro 没有 Responses Native。工具 strict、structured-output SSE 的完整组合、其他账号/区域、
强制 fallback、外部 SDK/Agent、负载和长期运行未证明。

## 相关文档

- [DeepSeek API 参考](../../references/providers/deepseek/api.md)
- [Models/能力预检](../features/models-api-and-capability-preflight.md)
