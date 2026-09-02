# DeepSeek 接入进度与边界

注册与能力事实见 `src/providers/deepseek/`；当前接线见[映射](../model-provider-mapping.md)。

## 当前边界

- Vision Files API/`file_id`、role/像素边界本地预检、600 图与 remote/mixed 64 MiB 极限、任意 remote host 可下载性、恶意图片、
  视觉质量与更高阶多能力组合未证明；inline executable profile 保守限制为累计 decoded 32 MiB。
- Pro 已记录官方 `low/high/max` 档位；普通 endpoint 不公开仅 `/beta` 保证的 function strict schema。
- `parallel_tool_calls` 请求控制、hosted/custom tool、structured-output SSE、强制 fallback、其他账号/区域和长期运行仍未证明。

## 验证与证据

- 2026-08-31 有界管理员 probe 覆盖 Chat；Bailian 侧 Responses 验证见 [bailian.md](bailian.md)。
- 2026-09-02 双协议矩阵实测（[evidence](../evidence/2026-09-02-dual-protocol-capability-matrix.md)）：
  vision-exp 实测仅接受 `tool_choice` auto/none，required/named/strict 与 `parallel_tool_calls` 显式控制全部 400；
  Chat json-schema 400、Responses json-schema 接受但不强制（与注册一致）。该 Target 注册的工具选择模式声明疑似过宽，待独立获准变更复核。
- 官方模型事实来源见 [references/providers/deepseek-api.md](../../references/providers/deepseek-api.md)。

## 代码 owner

`src/providers/deepseek/`。
