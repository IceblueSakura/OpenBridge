# Xiaomi MiMo Provider 状态

## 当前实现

- Provider family 为 `mimo`，固定 origin 为 `https://api.xiaomimimo.com`，使用 `mimo-primary` API-key pool。
- `mimo-v2.5-pro` 为 text-only Chat/Responses Native；`mimo-v2.5` 为 text/image Chat/Responses Native，并在 Chat 额外支持
  一个有界 WAV data URL 音频理解。
- `mimo-v2.5-asr`、`mimo-v2.5-tts`、`mimo-v2.5-tts-voicedesign` 与 `mimo-v2.5-tts-voiceclone`
  各自提供 Chat-only task profile，不生成 Bridge，也不继承通用文本模型的工具、structured output 或 reasoning 能力。
- 两个文本模型公开 `none/low/medium/high` 和明文 reasoning；Chat 将 `none` 映射为 disabled，其余档位映射为 enabled，
  Responses 保留具体 effort。
- 两个文本模型在 Chat/Responses 公开 function tool 与 `json_object`；只有 `mimo-v2.5` 公开
  `parallel_tool_calls`。四个专用音频模型对工具和 structured text fail closed。

## 所有权与确定性证据

- 注册与 wire 规则：[`src/providers/mimo/`](../../../src/providers/mimo/)。
- `tests/forwarding_contract.rs` 保护 text/image/audio wire、task 拒绝、parallel tool 与 structured-output 边界。
- `tests/provider_contract.rs`、`tests/provider_boundary_contract.rs` 保护相对 path、认证、header 和 Provider error。
- 功能级限制见 [Native 图片](../features/native-image-input.md)与 [MiMo 音频](../features/native-mimo-audio.md)。

## 真实 Provider 证据

定向真实请求确认两个文本模型、图片、`mimo-v2.5` 音频理解、ASR、TTS、VoiceDesign、VoiceClone 与 function tool；
音频输出可解码为 RIFF/WAV。标准 `/v1/audio/speech` 与 `/v1/audio/transcriptions` 对照均返回 404，当前实现因此只走
Chat task profile。

[2026-08-09 文字矩阵](../evidence/real-provider/2026-08-09-text-generation-none-high-matrix.md)覆盖两个文本模型的
Chat/Responses × JSON/SSE × `none/high`。定向 structured-output 结果支持 `json_object`，但 MiMo Pro Chat
`json_schema` 可能在生成不满足 schema 时以 abort 结束，Responses 则明确拒绝；当前不公开 JSON Schema。

## 未证明边界

`mimo-v2.5` video、remote/multiple audio、更多格式和上限、更多提示下的 parallel 稳定性、ASR 方言质量、TTS 音质、
外部 SDK/Agent、负载和长期运行未证明。HTTP 200 或多 tool call 不证明上游内部并发执行。

## 相关文档

- [MiMo API 参考](../../references/providers/xiaomi/api.md)
- [MiMo 音频参考](../../references/providers/xiaomi/audio.md)
- [MiMo 图片参考](../../references/providers/xiaomi/image.md)
