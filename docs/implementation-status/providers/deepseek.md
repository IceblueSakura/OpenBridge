# DeepSeek 接入进度与边界

注册与能力事实见 `src/providers/deepseek/`；当前 Target 与 Public Model 关系见[映射](../model-provider-mapping.md)。

## 特有接线与例外

- V4 Pro 与 V4.1 Flash 注册 Chat/Responses Native。
- Pro 记录官方 `low/high/max` 档位；普通 endpoint 不公开仅 `/beta` 保证的 function strict schema。

## 验证与证据入口

- [2026-08-29 Bailian DeepSeek V4 Pro Responses 接入验证](../evidence/2026-08-29-bailian-deepseek-v4-pro-responses.md)（Bailian source）
- 2026-08-31 有界管理员 probe 覆盖直接 DeepSeek Chat，没有提升为 SDK/Agent 或长期运行结论。
- 官方模型事实来源见 [references/providers/deepseek-api.md](../../references/providers/deepseek-api.md)。

## 代码 owner

`src/providers/deepseek/`。
