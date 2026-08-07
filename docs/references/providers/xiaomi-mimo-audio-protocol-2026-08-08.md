# Xiaomi MiMo ASR/TTS 协议与真实观察（复核于 2026-08-08）

## 来源与范围

本文只记录 MiMo ASR/STT 与 TTS 的官方 Chat-compatible wire 和 2026-08-08 脱敏真实 Provider 观察。它不把这些模型等同于
OpenAI 的 `/audio/*` API，也不证明 OpenBridge 当前已注册相应 Public Model。

- [语音识别 API](https://mimo.mi.com/docs/zh-CN/api/audio/Speech-Recognition)
- [语音识别指南](https://mimo.mi.com/docs/zh-CN/usage-guide/Speech-Recognition)
- [语音合成 API](https://mimo.mi.com/docs/zh-CN/api/audio/tts)
- [语音合成指南](https://mimo.mi.com/docs/zh-CN/usage-guide/speech-synthesis)
- [Models list](https://mimo.mi.com/docs/zh-CN/api/model/list-models)

## 官方事实

- Models list 包含 `mimo-v2.5-asr`、`mimo-v2.5-tts`、`mimo-v2.5-tts-voicedesign` 与
  `mimo-v2.5-tts-voiceclone`。
- ASR 与 TTS 都复用 `POST /v1/chat/completions`，不使用 `/audio/transcriptions` 或 `/audio/speech`。
- `mimo-v2.5-asr` 只接受一条 user `input_audio`：可以是带 `audio/wav`、`audio/mpeg` 或 `audio/mp3` MIME 的 data URL，
  也可以是同时携带 `format: "wav" | "mp3"` 的纯 Base64。官方声明 Base64 编码字符串不超过 10 MB。
- ASR 的可选 `asr_options.language` 为 `auto`、`zh` 或 `en`，默认 `auto`；JSON transcript 位于 assistant
  `message.content`，SSE 使用 Chat delta。usage 另含 `audio_tokens` 与音频秒数。
- `mimo-v2.5-tts` 把目标播报文本放在 assistant message；可选 user message 只提供风格/上下文。请求顶层 `audio` 选择 format 与
  voice，`mimo_default` 是默认预置音色，但实际音色会随部署集群映射。
- TTS API 声明 `wav`、`mp3`、`pcm`/`pcm16`；官方指南要求流式调用使用 `pcm16`。非流式响应在
  `message.audio.data` 返回 Base64 音频，流式响应在 `delta.audio.data` 返回 Base64 PCM16LE chunk；指南按 24 kHz、mono 拼接。
- voice design 与 voice clone 具有不同输入和数据保护边界；普通 TTS 成功不能证明这些模型或 voice sample wire 可用。

## 脱敏真实 wire 观察

先使用当前私有配置运行固定 MiMo Models probe；HTTP 200 的列表包含 ASR、TTS 与两个 TTS 变体。随后用同一个
`mimo-primary` pool 直连固定 Chat endpoint，执行完全在内存中的短中文 TTS→ASR 往返：

- 非流式 TTS 返回 HTTP 200、`application/json`、`chat.completion`、`mimo-v2.5-tts` 和 `finish_reason: "stop"`；
  `message.content` 为空，`message.audio.data` 是有效 WAV Base64。本次样本解码为 176,684 bytes、24 kHz、16-bit、mono、约
  3.68 秒。
- 非流式 ASR 使用上述 WAV data URL 与 `language: "zh"`，返回 HTTP 200、`mimo-v2.5-asr`、非空 transcript、
  `audio_tokens` 和 4 秒 usage；忽略标点后的 transcript 与目标文本完全一致。
- 流式 TTS 使用 `pcm16`，返回 `text/event-stream`；本次得到 12 个非空 audio delta、共 192,000 个 PCM bytes，最终
  `finish_reason: "stop"` 并出现 `[DONE]`。
- 流式 ASR 使用纯 Base64 + `format: "wav"` 与 `language: "auto"`，返回 `text/event-stream`；多个 text delta 拼接后
  与同一目标文本完全一致，最终 `finish_reason: "stop"` 并出现 `[DONE]`。

同日又按当前 OpenBridge API-key adapter 的认证形状，以 `Authorization: Bearer` 重复最小非流式 TTS→ASR 往返；两个请求均返回
HTTP 200，WAV 仍为 24 kHz、16-bit、mono，ASR 结果仍与目标文本一致。这确认了音频 Chat endpoint 接受未来 Route 会使用的 Bearer
header，但不代表 Route 已注册。

事件数、chunk 数、字节数和合成时长只是本次观察，不是稳定服务常量。测试没有落盘音频，也没有记录 credential、原始 Base64、完整
请求/响应、transcript 文本或 Provider request ID。六个音频请求均为直连 Provider；它们不证明当前 OpenBridge 可转发这两个模型。

## 证据边界

观察只覆盖一个账号、一个短中文样本、`mimo_default`、WAV/PCM16 与 JSON/SSE。未探测范围包括 MP3、英文/方言、其他预置音色、
voice design/clone、上限/非法输入、OpenAI SDK、当前 OpenBridge 路径、负载、长期运行和未来 Provider 状态。
