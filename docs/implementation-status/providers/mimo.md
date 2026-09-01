# Xiaomi MiMo 接入进度与边界

注册与能力事实见 `src/providers/mimo/`；当前接线见[映射](../model-provider-mapping.md)。

## 当前边界

- 独立 Chat structured-output 专页确认 MiMo-V2.5/Pro 的 JSON Object，但 Chat API reference 同时只列 text；当前只保留专页明确的
  Chat JSON Object。2026-09-01 已对 MiMo-V2.5 以固定 conflict prompt 完成管理员 probe，并以明确请求 JSON 的固定 prompt 完成
  真实下游 Gateway JSON/SSE 验收；这不证明 MiMo-V2.5-Pro、其他 schema/prompt、Responses structured output 或长期稳定性。
- Responses structured output、prompt-cache/include 不公开。
- video、remote/multiple audio、更多媒体格式和 limit、parallel 稳定性、ASR 方言质量、TTS 音质、外部 SDK/Agent、负载与长期运行未证明。
- 五种音频 task 的真实下游网关复测、播放器/硬件验收、负载和长期运行未完成。

## 验证与证据

- 2026-08-31 有界管理员 probe 覆盖 Chat；2026-09-01 MiMo-V2.5 Chat JSON Object JSON/SSE Gateway 验收（见 [current-state](../current-state.md)）。
- 官方模型事实来源见 [references/providers/xiaomi-api.md](../../references/providers/xiaomi-api.md)、
  [xiaomi-image.md](../../references/providers/xiaomi-image.md)、[xiaomi-audio.md](../../references/providers/xiaomi-audio.md)。

## 代码 owner

`src/providers/mimo/`。
