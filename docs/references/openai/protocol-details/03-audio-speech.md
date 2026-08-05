# OpenAI Text-to-speech 协议调研

## 1. Endpoint 与 response

Text-to-speech 使用独立 Audio Speech endpoint。request 是 JSON，核心语义包括 model、input text、voice 与输出音频格式；response
是音频 bytes 或音频 stream，不是 Chat/Responses JSON object。

资料：[Create speech](https://developers.openai.com/api/reference/resources/audio/subresources/speech/methods/create)、[Audio and speech](https://developers.openai.com/api/docs/guides/audio)、[Text to speech](https://developers.openai.com/api/docs/guides/text-to-speech)。

## 2. 协议维度

- voice 是受 model/Provider 约束的枚举或 identity；
- response format 决定 media type、文件扩展与 decoder；
- speed、instructions 等可选字段不能跨 model 假定；
- binary streaming 的首字节、取消和错误边界与 SSE semantic event 不同。

## 3. 边界

- TTS 是一次请求型 media generation，不等于 Realtime speech-to-speech session。
- 音频响应不可经 JSON parser 或文本 SSE renderer 处理。
- voice availability、format、最大 input 和合成模型会变化，需要按目标 model/profile 复核。
- 输出音频可能包含受版权、隐私或内容政策约束的数据；协议 schema 不替代使用政策。

