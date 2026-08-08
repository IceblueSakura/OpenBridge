# OpenAI Chat Completions 音频输入与输出调研

## 来源、范围与快照

本文只记录 `POST /v1/chat/completions` JSON/SSE 中的 audio input/output wire。`/v1/audio/*` 和 Realtime 分别由其他文档维护。

- 官方来源：[Audio and speech](https://developers.openai.com/api/docs/guides/audio)、[Create chat completion](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create)
- 官方资料复核日期：2026-08-08；动态 model、format、modalities 与 response shape 使用前仍须重核。

## 1. Audio input

user message content 中的 `input_audio` part 通常携带 Base64 audio data 与 format。part type、message/content 顺序、encoded bytes 与
format 都是 Chat wire 语义，不能替换为 text transcript 后声称无损。

## 2. Audio output

audio output 由 `modalities` 与顶层 `audio` 等字段请求，结果可位于 assistant message 的 audio 结构。非流式仍是 Chat completion
JSON；streaming 仍遵循 Chat choice/chunk terminal，而不是 `/audio/speech` 的裸 binary body。

## 3. 与专用 Audio/Realtime 的边界

- Chat audio 不自动提供 `/audio/transcriptions` 的 transcript schema；
- Chat audio output 不等于 `/audio/speech` binary response；
- Chat SSE 不等于 Realtime 双向 session；
- Responses 是否支持某种 audio item 必须按当期 Responses schema/model 单独确认，本文不推断。

## 4. 数据与证据边界

- Base64 audio、transcript、voice/audio config 与 output bytes 可能敏感；
- 一个 format/model success 不证明 audio-in/out 组合、stream、取消或全部 codec；
- Chat JSON/SSE fixture 不证明真实 audio decoder、playback 或 Provider compatibility。
