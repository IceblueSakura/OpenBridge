# Xiaomi MiMo 音频协议与固定观察

- Last reverified：外部来源与固定观察最后复核 2026-08-08；2026-08-24 仅整理本地文档，未刷新外部来源或重跑请求。
- Recheck trigger：音频 operation、Chat envelope、标准 Audio endpoint 或媒体限制变化。

## 来源与范围

- [Audio understanding](https://mimo.mi.com/docs/en-US/quick-start/usage-guide/multimodal-understanding/audio-understanding)
- [Speech recognition API](https://mimo.mi.com/docs/en-US/api/audio/Speech-Recognition)
- [Text-to-speech API](https://mimo.mi.com/docs/en-US/api/audio/Text-to-Speech)
- [Voice design](https://mimo.mi.com/docs/en-US/api/audio/Voice-Design)
- [Voice clone](https://mimo.mi.com/docs/en-US/api/audio/Voice-Clone)

本文只保留协议入口与 2026-08-08 的脱敏 endpoint 观察；逐模型能力、格式限制和音色参数以官方文档为准。

## 协议入口

MiMo 音频理解、ASR 与 TTS 使用 Chat Completions envelope，而不是标准 `/v1/audio/speech` 或 `/v1/audio/transcriptions`。具体 message content、task option 和 audio output 结构随官方音频操作定义变化。

## 固定观察

2026-08-08 的受控测试确认：Chat 路径可返回文本 transcript 或可解码 WAV；标准 OpenAI Audio Speech 与 Transcriptions 路径均返回 HTTP 404 HTML。该观察只证明当时账号、模型、短合成样本和网络，不证明其他格式、voice、时长、streaming、并发或长期可用性。

## 证据边界

本文不保存真实音频、credential、请求正文或完整响应。当前 OpenBridge 映射见[Model 与 Provider 映射](../../implementation-status/model-provider-mapping.md)。
