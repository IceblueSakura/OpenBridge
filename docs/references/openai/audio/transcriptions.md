# OpenAI Audio Transcriptions 调研

## 来源、范围与快照

本文只记录 `POST /v1/audio/transcriptions` 的 multipart audio upload 与 transcript response。Translation、TTS、Chat audio 与 Realtime
不在本文定义。

- 官方来源：[Create transcription](https://developers.openai.com/api/reference/resources/audio/subresources/transcriptions/methods/create)、[Speech to text](https://developers.openai.com/api/docs/guides/speech-to-text)
- 官方资料复核日期：2026-08-08；动态 model、format、limit 与 streaming option 使用前仍须重核。

## 1. Multipart request

request 至少包含 audio `file` 与 `model`。language、prompt、speaker labeling、timestamps、`stream` 和 `response_format` 取决于
具体 model 与当期 schema。

filename、part content type、file bytes、临时存储、body limit、取消和清理都属于 multipart wire 与资源边界。

## 2. Response forms

success 可为 JSON、纯文本、subtitle、详细 segment/timestamp 结构，或特定 transcription stream。`response_format` 改变 response
media type 与 parser，不能把所有输出统一当作 Chat text delta。

## 3. Retry 与数据保护

upload 可能已经被服务接受或计费；transport 结果不确定时不能假设任意重放安全。audio bytes、prompt、transcript、speaker/timestamp
信息可能敏感，不应进入普通日志。

## 4. 证据边界

- transcription 不等于 translation，也不证明 Chat/Realtime audio；
- 一个 format success 不证明其他 response format、stream、speaker/timestamp 或错误路径；
- mock multipart 不证明真实 codec、语言质量、大小或长期行为。
