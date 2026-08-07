# Native 音频能力需求

## 范围

本页定义两类 Chat Native 音频能力：标准 user `input_audio` 输入，以及独立 MiMo ASR/TTS task Public Model。它不实现 OpenAI
`/audio/speech`、`/audio/transcriptions`、`/audio/translations`、Responses audio 或 Realtime；共同规则见
[媒体扩展共同规则](embedding-and-native-multimodal.md)。当前尚无已完成的 Native audio 功能专题。

## 1. Chat `input_audio`

- `input_audio` 只在 user message content union 中有效；developer/system/tool/assistant 或任意递归同名字段必须拒绝。
- source 固定为 inline Base64；format 必须属于所选 interface 的明确集合，不能从 canonical audio modality 推断。
- `multimodal_input.audio` 必须公开 source、format、part 数、单项/累计 encoded/decoded byte 上限。
- Responses audio input 当前没有目标 wire，不能从 Chat 字段或 Provider 的一般音频能力推导。
- Native 转发保持 content part 顺序、Base64、format 与 Chat JSON/SSE；不得转写、转码、播放、落盘或缓存。

## 2. MiMo ASR/TTS 最小目标契约

MiMo 音频模型虽然都使用 `/v1/chat/completions`，但属于独立 canonical task、Public Model 与 Upstream API profile；不得继承
`mimo-v2.5` 文本/图片 Route，也不得通过 Provider 级 `audio_input`/`audio_output` bool 扩大其他模型能力。

| Public Model       | Native 请求契约                                                                                                                                  | Native 成功响应                                                                                                                |
|--------------------|--------------------------------------------------------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------------------|
| `mimo-v2.5-asr`    | 恰好一个 user `input_audio`；首个目标只接受 WAV，来源为 data URL 或 pure Base64 + `format: "wav"`；`asr_options.language` 只开放 `auto`/`zh`；JSON/SSE | JSON assistant `message.content` 或有序 Chat text delta；保留 `audio_tokens`、seconds、finish reason 与标准 Chat terminal     |
| `mimo-v2.5-tts`    | 可选 user 风格文本 + 恰好一个 assistant 目标文本；必需顶层 `audio`；JSON 只开放 `wav`，SSE 只开放 `pcm16`，voice 首个目标只开放 `mimo_default`             | JSON `message.audio.data` 保持 Base64 WAV；SSE `delta.audio.data` 保持有序 Base64 PCM16LE chunk，并以唯一 stop/`[DONE]` 结束 |

`asr_options` 与 `audio` 是对应 Chat interface 的顶层 typed parameter，只能由相应 Public Model 在 `supported_parameters` 中公开。TTS
assistant message 是待合成文本，不是普通历史；ASR 必须拒绝文本混入、多音频 part、非 user 角色或额外 message。

本目标只承诺真实 Provider 已覆盖的组合。官方另声明 ASR MP3/`en`、TTS MP3/其他 preset voice，但须以独立证据扩展固定 profile。
`mimo-v2.5-tts-voicedesign`、`mimo-v2.5-tts-voiceclone` 与 `mimo-v2.5` 通用音频理解不是这两个 Public Model 的 fallback 或别名。

## 3. `multimodal_output.audio` 与响应预算

音频输出不能使用粗粒度 `audio_output: true` 表达。Chat interface 必须提供类型化 `multimodal_output.audio`，至少区分：

- JSON/SSE mode 及各自允许的 request format/voice；
- response encoding/container、PCM endian、sample width、channels 与 sample rate；
- 单 event、非流式 JSON body 和累计 decoded audio 上限。

依赖 `stream` 才成立的 format 不能压平为无条件 allowed set。非流式 Base64 成功体必须在下游提交前受 JSON response hard limit
约束；SSE 只有 event limit 而没有累计 audio limit 时不得开放。

ASR inline bytes 同时受 typed profile 与 gateway request hard limit 约束；Provider 声明的 10 MB encoded limit 不会覆盖默认 1 MiB
request body limit，扩展 Models 必须公开实际更小的可保证值。

## 4. 预检、保真与 Bridge

- 请求分析冻结 message role/数量、source、encoding、input/output format、voice、ASR language、stream mode、part 数和 byte facts。
- ASR、TTS、通用音频理解与文本生成必须独立编译；共用 Provider 或 Chat path 不能聚合为 fallback 候选。
- ASR transcript 是该 task 的正常文本结果；TTS Base64 WAV/PCM delta 是正常音频结果，不能送入纯文本 validator、拼成
  transcript 或转换成 `/audio/speech` binary body。
- 网关只做有界 framing/shape 校验和 Public Model 投影，不解码后重采样、重编码、播放、落盘或缓存。
- Bridged Route 对 audio input/output 贡献空集；音频请求不得进入 Chat ↔ Responses Bridge 或按请求能力重排 Route。

## 5. Retry、取消与数据保护

- ASR 只有在 body 未超过 replay budget、响应尚未提交且仍是同一 target/model 时才能有限 retry。
- TTS 首个目标不自动 retry，因为再次合成可能重复计费并产生不同音频。
- 两者禁止跨 task/model fallback；任何 JSON body、text delta 或 audio delta 提交后不得 retry、rotation 重放或拼接响应。
- 原始音频、Base64、transcript、TTS 目标/风格文本、voice sample 和 Provider request ID 不得进入普通日志、metrics label、probe
  report 或 fixture。
- `audio_tokens`、seconds、audio bytes 与文本 token 必须保持语义，不把 PCM bytes 当 token、transcript 长度当时长或 chunk 数当速度。

## 6. 验收

| ID     | 应被保护的可观察行为                                                                                                                        |
|--------|---------------------------------------------------------------------------------------------------------------------------------------------|
| AUD-01 | Chat `input_audio` 只接受固定 role/source/format/limit；Responses audio 与未声明模型的 audio output 在 egress 前拒绝。                      |
| AUD-02 | `mimo-v2.5-asr` 的 WAV source/language/message contract、JSON/SSE transcript、usage、model 投影与单音频边界可确定复现。                    |
| AUD-03 | `mimo-v2.5-tts` 的 assistant/audio/voice contract、JSON WAV、SSE PCM16 chunk、累计预算、唯一 terminal 与取消可确定复现。                  |
| AUD-04 | ASR/TTS 是独立 Native-only Public Model，不提升 `mimo-v2.5`、进入 Bridge、跨 task fallback、启用 voice sample 或伪装成 `/audio/*`。       |
| AUD-05 | 独立客户端与真实 Provider 分别记录 endpoint、model、字段和证据边界；未运行 MP3、语言、音色、SDK、负载或长期层不声称通过。                 |

## 7. 非目标与参考

非目标包括 `/audio/*`、Responses audio、Realtime、通用音频理解、ASR MP3/`en`/方言承诺、TTS MP3/其他 preset voice、voice design 与
voice clone。

- [Chat/Responses 多模态协议调研](../references/openai/protocol-details/02-chat-responses-multimodal.md)
- [OpenAI Speech 调研](../references/openai/protocol-details/03-audio-speech.md)
- [OpenAI Transcription/Translation 调研](../references/openai/protocol-details/04-audio-transcription-translation.md)
- [Xiaomi MiMo ASR/TTS 协议与真实观察](../references/providers/xiaomi-mimo-audio-protocol-2026-08-08.md)
