# 功能：`mimo-v2.5` 音频理解与 MiMo 专用音频 Chat Native

## 当前行为

- `mimo-v2.5` Chat 支持一个有界 WAV data URL 的通用音频理解；Responses audio 关闭。
- `mimo-v2.5-asr`、`mimo-v2.5-tts`、`mimo-v2.5-tts-voicedesign` 与 `mimo-v2.5-tts-voiceclone`
  是独立 canonical task/Public Model，各自只有 Chat Native，不生成 Bridge/fallback。
- AudioUnderstanding 只接受 data URL/WAV/单 part，encoded/decoded 单项与累计上限为 10 MiB/8 MiB。
- ASR 接受单 user audio（data URL 或 pure Base64 WAV）与受限 language；TTS/VoiceDesign/VoiceClone 分别使用目标文本、风格/音色
  描述或独立 reference WAV conditioning。JSON 输出为 WAV，streaming 输出为 PCM16；普通 TTS preset 仅 `mimo_default`。
- 专用音频 Target 不公开 function tool 或 structured text；task-valid 请求携带这些字段在 egress 前 fail closed。
- Models 投影 task/source/format/voice/budget，不保存或解码音频。实现不下载、转码、重采样、播放、落盘、缓存或建立跨请求 voice identity。

## 所有权

Canonical facts/registration 位于 `src/models/xiaomi/`、`src/providers/mimo/` 和 `src/providers/catalog/public_models.rs`；audio
capability 位于 `src/core/capability/generation.rs`；shape analysis/preflight 位于 `src/pipeline/analysis/generation/audio.rs` 与
`src/pipeline/preflight.rs`。

## 确定性与真实证据

`tests/forwarding_contract.rs` 覆盖五种 task 的 Models、JSON/SSE exact wire、conditioning、message shape、source/format/budget、
工具/structured output 和 zero-egress mismatch。

真实 MiMo 直连请求确认通用音频理解、短 WAV ASR transcript，以及 TTS/VoiceDesign/VoiceClone 返回可解码 RIFF/WAV；四个专用
模型携带 required tool 的对照没有产生 tool call，因此当前 fail-closed。完整边界见 [MiMo Provider 状态](../providers/mimo.md)。

## 未证明范围

OpenAI `/v1/audio/*`、Responses audio、Realtime、五种 task 的真实下游网关复测、remote/multiple audio、更多格式/语言/voice、
媒体质量、voice authorization/store、外部 SDK/Agent、负载、长期运行和播放器/硬件验收未证明。

## 相关文档

- [Native 音频需求](../../functional-requirements/extended-capabilities/native-audio.md)
- [MiMo 音频参考](../../references/providers/xiaomi/audio.md)
- [MiMo Provider 状态](../providers/mimo.md)
