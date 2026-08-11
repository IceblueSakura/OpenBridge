# Xiaomi MiMo 全模型语音能力与调用途径（复核于 2026-08-08）

## 来源与范围

本文按当前公共 Models API 返回的六个 MiMo model ID，记录每个模型是否具有语音能力、任务语义、调用入口、关键 wire 和证据等级。
标准 `/audio/*`、Chat audio 与 Realtime 的协议定义分别见
[OpenAI 音频与语音索引](../../openai/README.md#6-音频与语音)和
[Realtime 索引](../../openai/README.md#7-realtime)，不在本文重复定义。

本文是外部 Provider 快照，不代表 OpenBridge 已注册或开放这些模型。动态 model list、format、voice、limit 与服务行为会变化。

官方来源：

- [Models list](https://mimo.mi.com/docs/en-US/api/model/list-models)
- [Audio understanding](https://mimo.mi.com/docs/en-US/quick-start/usage-guide/multimodal-understanding/audio-understanding)
- [Speech recognition API](https://mimo.mi.com/docs/en-US/api/audio/Speech-Recognition)
- [Speech recognition guide](https://mimo.mi.com/docs/en-US/usage-guide/Speech-Recognition)
- [Speech synthesis API](https://mimo.mi.com/docs/en-US/api/audio/tts)
- [MiMo-V2.5-TTS series guide](https://mimo.mi.com/docs/en-US/quick-start/usage-guide/audio/speech-synthesis-v2.5)

证据标记：

- **官方**：当前官方文档或 Models API 明确声明；
- **实测**：使用当前本地私有 credential 直连公共 Provider，且只记录脱敏 metadata；
- **未实测**：不能由相邻模型或相似字段的成功结果外推。

## 1. 六模型能力矩阵

| Model ID                         | 语音任务与支持结论                         | 输入 → 输出                              | Provider 调用途径                                                                 | 证据                         |
|----------------------------------|--------------------------------------------|-------------------------------------------|-----------------------------------------------------------------------------------|------------------------------|
| `mimo-v2.5-pro`                  | 未声明语音理解、ASR 或 TTS                 | 无语音 contract                           | 官方音频理解页没有把该模型列为支持模型；其文本生成入口不构成语音途径              | Models 实测；官方负向边界    |
| `mimo-v2.5`                      | 通用音频理解                               | audio + instruction → Chat 文本/推理结果 | `POST /v1/chat/completions`；user `input_audio` 与 text content part              | Models、短 WAV 均实测        |
| `mimo-v2.5-asr`                  | 专用 ASR/STT                               | speech audio → transcript                 | `POST /v1/chat/completions`；单个 user `input_audio` + 顶层 `asr_options`          | Models、JSON/SSE 均实测      |
| `mimo-v2.5-tts`                  | 预置音色 TTS                               | target text + style → audio               | `POST /v1/chat/completions`；user style + assistant target + 顶层 `audio`          | Models、JSON/SSE 均实测      |
| `mimo-v2.5-tts-voicedesign`      | 文本描述设计新音色并合成                   | voice description + target text → audio   | `POST /v1/chat/completions`；user voice description + assistant target + `audio`  | Models、非流式 WAV 均实测    |
| `mimo-v2.5-tts-voiceclone`       | 参考音频克隆音色并合成                     | reference audio + target text → audio     | `POST /v1/chat/completions`；assistant target + `audio.voice` 中的 audio data URL  | Models、非流式 WAV 均实测    |

当前 `GET /v1/models` 的官方示例与 2026-08-08 真实请求均只返回上述六个 ID。早期已下线模型、未进入该账号目录的 early-access
模型和未来新增模型不在“全模型”范围内。

## 2. `mimo-v2.5`：通用音频理解

官方只把 `mimo-v2.5` 列为 audio understanding model，因此不能把 `mimo-v2.5-pro` 或任一专用 ASR/TTS 模型视为同任务 fallback。

调用形状：

- endpoint 是 `POST /v1/chat/completions`；
- user content 中放置一个或多个 `input_audio`，并与 text instruction 保持有序组合；
- source 可以是公网 URL 或带 MIME prefix 的 Base64 data URL，不是 multipart upload；
- 官方列出 MP3、WAV、FLAC、M4A、OGG；URL 单文件不超过 100 MB，单个 Base64 encoded string 不超过 50 MB；
- 结果用于回答、总结、分析或推理，不提供专用 transcript fidelity contract，也不输出合成音频。

官方示例当前显示空 `message.content` 与非空 `reasoning_content`。2026-08-08 使用内存中的短合成 WAV 直连公共 Provider，Chat 请求
返回 HTTP 200 和与任务相符的文本/推理结果；单个样本仍不足以确定稳定的 JSON/SSE 可见输出分类、完整格式矩阵或质量上界。

## 3. `mimo-v2.5-asr`：专用语音识别

ASR 仍使用 Chat Completions，而不是 `/v1/audio/transcriptions`：

- 恰好一个 user `input_audio`；官方不允许把多个音频或普通 text part 混入该任务；
- data 可以是 WAV/MP3 data URL，或 pure Base64 + 匹配的 `format: "wav" | "mp3"`；encoded string 不超过 10 MB；
- 顶层 `asr_options.language` 支持 `auto`、`zh`、`en`；
- JSON transcript 位于 assistant `message.content`，SSE 使用 Chat text delta；
- usage 另含 audio token 与音频 seconds。

2026-08-08 使用内存中的短中文 WAV 直连公共 Provider：非流式和流式请求均返回 HTTP 200，transcript 非空且与目标文本一致；SSE
最终出现 `finish_reason: "stop"` 与 `[DONE]`。Bearer 与 `api-key` 两种官方认证形状都完成过非流式正向验证。

## 4. MiMo-V2.5-TTS 三个模型

三个 TTS 模型共享以下 Chat wire：目标播报文本通常放在 assistant message，user message 放风格、音色描述或上下文；顶层 `audio`
选择输出 format 及模型特有条件。非流式 audio 位于 `message.audio.data`，流式 chunk 位于 `delta.audio.data`。

| 模型                             | 音色来源                                      | 专有字段与限制                                                                 | Streaming 状态                                      |
|----------------------------------|-----------------------------------------------|--------------------------------------------------------------------------------|-----------------------------------------------------|
| `mimo-v2.5-tts`                  | `audio.voice` 选择内置 voice                  | `mimo_default` 随部署集群映射；支持自然语言风格、audio tag 和 singing          | 官方声明低延迟 streaming 可用；使用 `pcm16`         |
| `mimo-v2.5-tts-voicedesign`      | 必需 user message 以自然语言描述新音色        | 不接收参考音频或内置 voice；可用 `optimize_text_preview`，特定模式可省略 target | 可用兼容式 stream，但官方明确不是低延迟输出         |
| `mimo-v2.5-tts-voiceclone`       | `audio.voice` 接收参考音频 data URL           | 参考音频只支持 WAV/MP3，Base64 encoded string 不超过 10 MB                    | 可用兼容式 stream，但官方明确不是低延迟输出         |

普通 TTS 的非流式真实请求返回 HTTP 200、`chat.completion`、空 text content 与有效 Base64 WAV；流式 `pcm16` 返回多个有序
audio delta，观察到 24 kHz、16-bit、mono PCM，最终出现 `finish_reason: "stop"` 与 `[DONE]`。VoiceDesign 与 VoiceClone 的短样本
非流式请求也返回可解码 RIFF/WAV；这些结果不证明它们的 streaming wire，也不把一次样本的 chunk 数、音频长度或音色映射提升为
稳定常量。

VoiceClone 的 reference audio 是 voice conditioning，不是待理解或待转写的业务音频；其授权、保留、日志和隐私边界也不能从普通 TTS
继承。

## 5. 实际 endpoint 支持

MiMo 官方 API Reference 虽把 ASR/TTS 放在 Audio 导航下，但两页的 request address 都是
`https://api.xiaomimimo.com/v1/chat/completions`。“OpenAI API Compatibility”在这里描述 Chat SDK/request envelope，不代表
OpenAI Audio endpoint 兼容。

同一 credential、Bearer 认证和公共 origin 的正负对照如下：

| Probe                                      | 结果                                                                                                      |
|--------------------------------------------|-----------------------------------------------------------------------------------------------------------|
| `GET /v1/models`                           | HTTP 200；返回六个 model ID                                                                               |
| Chat `mimo-v2.5` 音频理解                 | HTTP 200；短 WAV 返回与任务相符的文本/推理结果                                                            |
| Chat `mimo-v2.5-tts`                       | HTTP 200；返回可解码 RIFF/WAV                                                                             |
| Chat `mimo-v2.5-asr`                       | HTTP 200；返回非空 transcript、audio tokens 与 seconds                                                    |
| Chat `mimo-v2.5-tts-voicedesign`           | HTTP 200；返回可解码 RIFF/WAV                                                                             |
| Chat `mimo-v2.5-tts-voiceclone`            | HTTP 200；返回可解码 RIFF/WAV                                                                             |
| `POST /v1/audio/speech`                    | HTTP 404、`text/html`、OpenResty `404 Not Found`                                                          |
| `POST /v1/audio/transcriptions`            | HTTP 404、`text/html`、OpenResty `404 Not Found`                                                          |

因此，截至复核日期，可以确认当前公共 MiMo API origin 对本次账号没有暴露这两个 OpenAI Audio 路由；失败发生在反向代理路由层，而不是
JSON model/parameter 校验层。该结论不能证明未来版本、其他 region、私有部署或未公开 endpoint 永远相同。

## 6. 证据边界

- ASR/TTS 实测只覆盖一个账号、一个短中文样本、`mimo_default`、WAV/PCM16 与 JSON/SSE；
- 通用音频理解、VoiceDesign 与 VoiceClone 也只覆盖短合成样本和非流式正向路径；remote/multi-audio、MP3、英文/方言、其他预置音色、
  上限与非法输入仍未做真实 Provider 验证；
- `/audio/translations` 未探测，因为当前 MiMo model list 和官方文档没有对应的 translation task/model；
- 未安装外部 OpenAI SDK；Audio endpoint 探测直接发送其标准 HTTP request shape；
- 没有记录 credential、原始 Base64、完整 transcript、完整请求/响应或 Provider request ID；
- Provider 正向或负向结果均不证明 OpenBridge 当前实现、外部 SDK、负载、长期运行或未来服务状态。
