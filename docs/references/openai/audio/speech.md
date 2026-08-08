# OpenAI Audio Speech 调研

## 来源、范围与快照

本文只记录 `POST /v1/audio/speech` 的 JSON request 与 binary/streaming audio response。Transcription、translation、Chat audio 和
Realtime 不在本文定义。

- 官方来源：[Create speech](https://developers.openai.com/api/reference/resources/audio/subresources/speech/methods/create)、[Text to speech](https://developers.openai.com/api/docs/guides/text-to-speech)
- 官方资料复核日期：2026-08-08；动态 model、voice、format 与 limit 使用前仍须重核。

## 1. JSON request

核心字段包括 `model`、待合成 `input` 与 `voice`；`instructions`、language、speed 或输出 format 是否可用取决于目标
model/profile。

voice 是受 model/Provider 约束的 enum 或 identity，不能假定跨 model 或 Provider 可移植。input 与 voice 也属于可能敏感的数据。

## 2. Binary/stream response

success 返回 audio bytes 或可边收边播放的音频流，不是 Chat completion JSON，也不是 text SSE。兼容实现需要保持 response
`Content-Type`、container/codec、format/extension、byte stream、decoder 与 body budget。

首个 audio byte 提交后发生的错误、取消与不可重放边界必须按媒体 response 处理，不能拼接第二次 attempt。

## 3. 与其他音频 operation 的边界

- file transcription 见 [Transcriptions](transcriptions.md)；
- file translation 见 [Translations](translations.md)；
- Chat audio 见 [Chat 音频输入/输出](chat-input-output.md)；
- 实时 speech-to-speech 见 [Realtime transport](../realtime/transport.md)。

## 4. 证据边界

- Speech generation 不提供 ASR transcript contract；
- 一个 voice/format success 不证明全部 enum、stream、取消或真实 decoder；
- schema 不替代版权、隐私或内容政策。
