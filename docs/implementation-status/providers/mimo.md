# Xiaomi MiMo 接入进度与边界

注册与能力事实见 `src/providers/mimo/`；当前 Target 与 Public Model 关系见[映射](../model-provider-mapping.md)。

## 特有接线与例外

- MiMo-V2.5/Pro 的独立 structured-output 专页确认 Chat JSON Object，但 Chat API reference 同时只列 text；当前只保留专页明确的 Chat JSON Object。2026-09-01 对 MiMo-V2.5 以 conflict prompt 完成管理员 probe，并以明确 JSON prompt 完成真实下游 Gateway JSON/SSE 验收，不覆盖 Pro、其他 schema/prompt、Responses structured output 或长期稳定性。
- Responses structured output、prompt-cache/include 不公开。
- video、remote/multiple audio、更多媒体格式和 limit、parallel 稳定性、ASR 方言质量、TTS 音质未验证；五种音频 task 的真实下游网关复测、播放器/硬件验收、负载和长期运行未完成。
- 2026-09-02 矩阵中 Chat structured/tool case 均取得结果但部分未命中固定 oracle；Responses `reasoning-max` 与 json-schema/json-schema-strict 被 400 拒绝，后三个 Responses-only 差分 case 被接受。该 reasoning 上限是待独立复核的注册差异。

## 验证与证据入口

- [2026-09-02 双协议能力探测矩阵](../evidence/2026-09-02-dual-protocol-capability-matrix.md)
- 2026-08-31 有界管理员 probe 覆盖 Chat；2026-09-01 的 MiMo-V2.5 JSON/SSE Gateway 验收保留在本页，未另建重复 probe 报告。
- 官方模型事实来源见 [references/providers/xiaomi-api.md](../../references/providers/xiaomi-api.md)、[xiaomi-image.md](../../references/providers/xiaomi-image.md)、[xiaomi-audio.md](../../references/providers/xiaomi-audio.md)。

## 代码 owner

`src/providers/mimo/`。
