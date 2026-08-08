# OpenAI Audio Translations 调研

## 来源、范围与快照

本文只记录 `POST /v1/audio/translations` 的 multipart audio upload 与语音翻译 response。Transcription、TTS、Chat audio 与 Realtime
不在本文定义。

- 官方来源：[Create translation](https://developers.openai.com/api/reference/resources/audio/subresources/translations/methods/create)、[Speech to text](https://developers.openai.com/api/docs/guides/speech-to-text)
- 官方资料复核日期：2026-08-08；动态 model、format 与 limit 使用前仍须重核。

## 1. Operation semantics

translation 是语音翻译 operation，不是保留源语言的逐字 transcription。当前资料把文件翻译描述为生成英文译文的独立路径；目标
model、语言边界和 response format 必须按当期资料确认。

## 2. Multipart 与 response

request 使用 multipart audio file，并可包含 operation/profile 允许的其他 fields。filename、content type、bytes、temporary storage、
body limit 与 cleanup 都是 wire/resource contract。

response 可为该 operation 支持的 JSON/text/subtitle 形状；不能从 Transcriptions endpoint 机械继承所有 field 或 format。

## 3. Retry、数据与证据边界

- upload 可能已被接受或计费，transport 不确定时不能盲目重放；
- audio 与 translation text 可能敏感；
- 一个语言/format sample 不证明其他输入语言、response format、错误或模型质量；
- translation success 不证明 transcription、Chat audio 或 Realtime。
