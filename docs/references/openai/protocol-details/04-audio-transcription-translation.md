# OpenAI Audio Transcription/Translation 协议调研

## 1. Endpoint 差异

Transcription 与 Translation 是两个独立 endpoint。两者都接收 audio upload 和 model；transcription 保留/识别源语言，translation 产生目标语言文本，支持字段和模型范围不必相同。

资料：[Create transcription](https://developers.openai.com/api/reference/resources/audio/subresources/transcriptions/methods/create)、[Create translation](https://developers.openai.com/api/reference/resources/audio/subresources/translations/methods/create)、[Speech to text](https://developers.openai.com/api/docs/guides/speech-to-text)。

## 2. Multipart 与 response format

- request 使用 `multipart/form-data`，包含 binary file 与普通 fields；
- filename、content type、file bytes 和其他字段都参与实际 wire；
- response format 可影响 JSON/text/SRT/VTT/verbose JSON 等结果形状；
- 某些 model/profile 支持 streaming transcription，但其 event 与普通 SSE generation 不相同。

## 3. 边界

- upload body size、audio duration、format 和 language support 是 model/service profile 事实。
- multipart parser 的临时存储、取消、超限和清理属于重要资源边界。
- 同一音频请求通常具有副作用/成本，transport 不确定时不能假定任意重放安全。
- transcript 可能包含敏感语音内容，日志与长期保留需要单独政策。

