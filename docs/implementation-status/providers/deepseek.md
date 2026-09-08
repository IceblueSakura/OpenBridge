# DeepSeek 接入进度与边界

注册与能力事实见 `src/providers/deepseek/`；当前 Target 与 Public Model 关系见[映射](../model-provider-mapping.md)。

## 特有接线与例外

- V4 Pro、V4 Flash 和 V4 Flash Vision Exp 注册 Chat/Responses Native；Vision 的 inline executable profile 保守限制为累计 decoded 32 MiB。
- Vision Files API/`file_id`、role/像素边界本地预检、600 图与 remote/mixed 64 MiB 极限、任意 remote host 可下载性、恶意图片、视觉质量和高阶多能力组合未证明。
- Pro 记录官方 `low/high/max` 档位；普通 endpoint 不公开仅 `/beta` 保证的 function strict schema。
- 2026-09-02 矩阵中 Vision Exp 仅接受 `tool_choice` auto/none，required/named/strict 与 `parallel_tool_calls` 显式控制全部 400；Chat json-schema 400、Responses json-schema 接受但不强制，与注册收窄一致。该 Target 的工具选择模式声明疑似过宽，待独立获准变更复核。

## 验证与证据入口

- [2026-09-02 双协议能力探测矩阵](../evidence/2026-09-02-dual-protocol-capability-matrix.md)
- [2026-08-29 Bailian DeepSeek V4 Pro Responses 接入验证](../evidence/2026-08-29-bailian-deepseek-v4-pro-responses.md)（Bailian source）
- 2026-08-31 有界管理员 probe 覆盖直接 DeepSeek Chat；没有把该 probe 提升为真实 SDK、其他账号/区域或长期运行结论。
- 官方模型事实来源见 [references/providers/deepseek-api.md](../../references/providers/deepseek-api.md)。

## 代码 owner

`src/providers/deepseek/`。
