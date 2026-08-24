# Native 音频能力需求

## 范围

本页定义五种不可互换的 Chat Native 音频任务：通用音频理解、ASR/STT、TTS、以文本约束音色的 VoiceDesign，
以及以参考音频约束音色的 VoiceClone。本页不定义 OpenAI `/audio/speech`、`/audio/transcriptions`、
`/audio/translations`、Responses audio 或 Realtime；共同规则见[扩展共同规则](README.md)，实现与验证事实见
[实施现状](../../implementation-status/README.md)。

## 1. 任务身份与不可替代性

| 任务                    | MiMo model family                         | 输入到输出                         | 输入音频的业务语义                         |
|-------------------------|-------------------------------------------|------------------------------------|--------------------------------------------|
| 通用音频理解            | `mimo-v2.5`                               | audio + instruction → text answer | 需要问答、总结、描述或推理的业务内容       |
| ASR/STT                 | `mimo-v2.5-asr`                           | speech audio → transcript          | 需要尽量忠实转写的语音                     |
| 普通 TTS                | `mimo-v2.5-tts`                           | target text + style → audio        | 不接收待理解音频，只生成语音               |
| 音色设计                | `mimo-v2.5-tts-voicedesign`               | voice description + text → audio  | 不接收参考音频，以文本创建音色             |
| 音色克隆                | `mimo-v2.5-tts-voiceclone`                | reference audio + text → audio    | 只提取说话人音色，不回答或转写参考音频内容 |

这些任务即使共用 `/v1/chat/completions`、`input_audio` 或 Chat response envelope，也不得合并 canonical task、Public Model、
Upstream API profile、能力交集、计费语义或 fallback 候选。通用模型被提示“转写”不等于 ASR transcript contract；voice sample
也不等于可供问答的音频内容。

Canonical Model 使用必填 task union：通用音频理解仍属于 `Generation`，其余四类分别使用 `SpeechRecognition`、
`SpeechSynthesis`、`VoiceDesign`、`VoiceClone`。Provider audio ceiling 与 Target executable profile 是不同静态类型：ceiling 是
非空、task 不重复的完整 profile 集合，每个元素携带自己的 input/output/conditioning/delivery 上界；一个 Chat Upstream API
只能省略 audio，或绑定一个 `AudioUnderstanding | SpeechRecognition | SpeechSynthesis | VoiceDesign | VoiceClone` concrete profile。
这两个闭合类型是 Provider 多任务上界与单 Target 可执行能力的唯一表示。

## 2. `mimo-v2.5` 通用音频理解

- 首个协议目标只开放 Chat user message content 中的 `input_audio`，可与同一 user message 中的 text part 混合；Responses audio
  仍无目标 wire。
- 官方能力上界包括公网 URL 与 Base64 data URL、MP3/WAV/FLAC/M4A/OGG 和多个音频；Public Model 只能公开固定 Route 已有独立
  完整 contract 与本地有界校验的 source、media type、part 数和 limits，不能直接照搬 Provider 上界。
- 当前 executable profile 只开放一个 WAV data URL，单项与累计 encoded/decoded 上限分别为 10 MiB/8 MiB；remote URL、pure
  Base64、其他格式和多个 audio part 均保持关闭。部署级 request hard limit 继续独立生效。
- `multimodal_input.audio` 必须公开业务用途 `content_understanding`、source、inline encoding、可验证 media type、part 数、URL
  长度及单项/累计 encoded/decoded byte 上限。
- remote source 服从有界 absolute HTTPS 与本地地址拒绝策略；OpenBridge 不下载音频，因此不能把语法检查冒充 Provider-side
  DNS、redirect、下载大小、MIME 或内容安全验证。
- Native 转发保持 audio/text part 顺序、URL/data URL、Chat JSON/SSE 与模型响应字段；不得预先转写、转码、重采样、播放、落盘、
  缓存或把音频替换成 transcript。
- 正常结果是依据音频和 instruction 生成的文本回答，而不是逐字 transcript 或音频输出；`asr_options`、顶层 `audio` 与 voice sample
  字段在该 interface 上必须拒绝。

## 3. MiMo ASR/TTS 最小目标契约

MiMo 音频模型虽然都使用 `/v1/chat/completions`，但属于独立 canonical task、Public Model 与 Upstream API profile；不得继承
`mimo-v2.5` 文本/图片 Route。Provider ceiling 只能限制 Target 的 complete executable profile，不能直接成为 Route profile，也不能
授予其他 task 能力；audio presence 与 input/output/conditioning 必须从 concrete variant 派生。

| Public Model       | Native 请求契约                                                                                                                                  | Native 成功响应                                                                                                                |
|--------------------|--------------------------------------------------------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------------------|
| `mimo-v2.5-asr`    | 恰好一个 user `input_audio`；首个目标只接受 WAV，来源为 data URL 或 pure Base64 + `format: "wav"`；`asr_options.language` 只开放 `auto`/`zh`/`en`；JSON/SSE | JSON assistant `message.content` 或有序 Chat text delta；保留 `audio_tokens`、seconds、finish reason 与标准 Chat terminal     |
| `mimo-v2.5-tts`    | 可选 user 风格文本 + 恰好一个 assistant 目标文本；必需顶层 `audio` 与 `format`；JSON 只开放 `wav`，SSE 只开放 `pcm16`；`voice` 可省略，显式提供时首个目标只开放 `mimo_default` | JSON `message.audio.data` 保持 Base64 WAV；SSE `delta.audio.data` 保持有序 Base64 PCM16LE chunk，并以唯一 stop/`[DONE]` 结束 |

`asr_options` 与 `audio` 是对应 Chat interface 的顶层 typed parameter，只能由相应 Public Model 在 `supported_parameters` 中公开。TTS
assistant message 是待合成文本，不是普通历史；ASR 必须拒绝文本混入、多音频 part、非 user 角色或额外 message。

本契约只允许固定 profile 明确列出的组合；其他 format/language/voice 必须先扩展该 profile，不能从 Provider 上界自动继承。
`mimo-v2.5-tts-voicedesign`、`mimo-v2.5-tts-voiceclone` 与 `mimo-v2.5` 通用音频理解不是这两个 Public Model 的 fallback 或别名。

## 4. 音色设计与音色克隆边界

- 音色设计的自然语言描述是生成条件，不得作为普通 TTS 可选 voice 名称或通用模型 instruction 处理。
- 音色克隆的参考音频是 `voice_conditioning` resource，不得进入 `content_understanding` 或 `speech_recognition` profile；固定 contract 只
  暴露独立 source/format/byte limit，授权确认、保留期和日志脱敏策略仍是后续媒体治理边界。
- 两个变体各自使用独立 canonical task、Chat Native profile 与失败边界；普通 TTS 成功不提升其他模型能力。
- VoiceDesign 只接受自然语言 voice description；VoiceClone 只接受独立 `audio.voice` reference resource，不建立跨模型
  voice identity 或资源复用。gateway 只做 shape/source/format/size 预检，不承诺授权、保留期或媒体内容验证。

## 5. `multimodal_output.audio` 与响应预算

音频输出不能使用粗粒度 bool 表达。Chat interface 必须提供类型化 `multimodal_output.audio`，至少区分：

- JSON/SSE mode 及各自允许的 request format/voice；
- response encoding/container、PCM endian、sample width、channels 与 sample rate；
- 单 event、非流式 JSON body 和累计 decoded audio 上限。

当前获准的 generated-audio executable profile 必须同时携带完整 JSON delivery 与完整 SSE delivery；二者都不是 `Option`，各自的
format 集合必须非空、budget 必须为正，并固定自己的 framing。只支持 JSON 或只支持 SSE 是未来未获准 contract，不能用空集合、零值
或缺失 payload 预占。普通 TTS 另拥有非空 preset voice 集合；VoiceDesign/VoiceClone 不使用空 voice 集合哨兵，VoiceClone 另有必填
conditioning profile。TTS downstream `audio.voice` 仍可省略；省略不等于配置中存在一个空 voice。

依赖 `stream` 才成立的 format 不能压平为无条件 allowed set。非流式 Base64 成功体必须在下游提交前受 JSON response hard limit
约束；SSE 只有 event limit 而没有累计 audio limit 时不得开放。

ASR inline bytes 同时受 typed profile 与 gateway request hard limit 约束；Provider 声明的 10 MB encoded limit 不会覆盖部署配置的 100 MiB
request body limit，扩展 Models 必须公开实际更小的可保证值。

## 6. 启动门禁、预检、保真与 Bridge

- 请求分析只冻结任务无关的协议结构，不决定业务 task：`RequestedAudio::Input` 保存 resources、
  `InputAudioMessageShape::SingleUserAudioOnly | GeneralConversation` 与
  `RequestedAsrOptions::Absent | Present { language }`；`RequestedAudio::Generated` 保存 delivery、
  `GeneratedAudioMessageShape::AssistantTextOnly | UserTextThenAssistantText | Other` 与
  `RequestedVoice::Unspecified | Preset | ReferenceVoice`。analyzer 不查询 registry、不选择 Public Model interface/Route，也不把
  user text 提前解释为 TTS style 或 VoiceDesign description。
- Public Model preflight 解析所选的 concrete audio interface 后，才以双 enum match 解释 AudioUnderstanding、ASR、TTS、VoiceDesign
  与 VoiceClone 的 role、text、language、voice 和 conditioning 语义；AudioUnderstanding 接受通用 conversation shape，ASR 只接受
  `SingleUserAudioOnly`，VoiceClone 只接受 `AssistantTextOnly`，TTS 接受 `AssistantTextOnly` 或
  `UserTextThenAssistantText`，VoiceDesign 只接受 `UserTextThenAssistantText`。`Other` 必须 fail closed，不改选模型或候选。
- 启动时先依赖 checked constructors 保证每个 primitive/profile 完整，再验证 executable profile 是 Provider ceiling 中同 variant 的
  payload subset，最后校验 canonical task/profile matrix。专用 canonical task 缺 profile 或绑定不同 variant 必须失败；Generation
  只有在 input modalities 明确含 Audio 且 output modalities 明确含 Text 时才可绑定 AudioUnderstanding，未知 evidence 失败关闭。
- ASR、TTS、音色条件和通用音频理解必须独立编译；`mimo-v2.5` 的普通 text/audio 生成仍属于同一固定 Chat interface，但不能与
  专用模型聚合为 fallback 候选。
- ASR transcript 是该 task 的正常文本结果；TTS Base64 WAV/PCM delta 是正常音频结果，不能送入纯文本 validator、拼成
  transcript 或转换成 `/audio/speech` binary body。
- 网关只做有界 framing/shape 校验和 Public Model 投影，不解码后重采样、重编码、播放、落盘或缓存。
- Bridged Route 对 audio input/output 贡献空集；音频请求不得进入 Chat ↔ Responses Bridge 或按请求能力重排 Route。

## 7. Retry、取消与数据保护

- 通用音频理解只在 body 未超过 replay budget、响应尚未提交且仍是同一 target/model 时有限 retry；不得 fallback 到 ASR。
- ASR 只有在 body 未超过 replay budget、响应尚未提交且仍是同一 target/model 时才能有限 retry。
- TTS 首个目标不自动 retry，因为再次合成可能重复计费并产生不同音频。
- 所有音频任务禁止跨 task/model fallback；任何 JSON body、text delta 或 audio delta 提交后不得 retry、rotation 重放或拼接响应。
- 原始音频、Base64、transcript、TTS 目标/风格文本、voice sample 和 Provider request ID 不得进入普通日志、metrics label、probe
  report 或 fixture。
- `audio_tokens`、seconds、audio bytes 与文本 token 必须保持语义，不把 PCM bytes 当 token、transcript 长度当时长或 chunk 数当速度；
  gateway 只保留并透传上游 JSON/SSE，不自行计算或重解释这些字段。

## 8. 验收

| ID     | 应被保护的可观察行为                                                                                                                          |
|--------|-----------------------------------------------------------------------------------------------------------------------------------------------|
| AUD-01 | Chat 音频能力按 understanding、ASR、TTS、VoiceDesign、VoiceClone 分开公开；Responses audio 和未声明模型的 audio output 在 egress 前拒绝。 |
| AUD-02 | `mimo-v2.5` 只在固定 Chat Native interface 接受已声明 source/format/limit，保持 mixed audio/text wire，并返回文本回答而非 transcript/audio。 |
| AUD-03 | `mimo-v2.5-asr` 的 WAV source/language/message contract、JSON/SSE transcript、usage、model 投影与单音频边界可确定复现。                    |
| AUD-04 | `mimo-v2.5-tts` 的 assistant/audio/voice contract、JSON WAV、SSE PCM16 chunk、累计预算、唯一 terminal 与取消可确定复现。                  |
| AUD-05 | voice design/clone 使用独立条件输入、输出 contract 和失败边界；只开放有界 Chat profile，不建立授权存储、voice identity 或资源复用。 |
| AUD-06 | 音频请求不进入 Bridge、跨 task fallback、请求期候选筛选，或伪装成 `/audio/*`；首输出 commit 后不发生第二次响应。                           |
| AUD-07 | 每个 task 的 endpoint/model/source/format 都由固定 profile 单独拥有；一个 task 的成功不能提升另一 task 的能力。                |
| AUD-08 | Provider 完整 profile ceiling、单个 executable profile 与 canonical task 依次通过启动门禁；多任务上界不进入单 Target 或跨 task 聚合。 |
| AUD-09 | analyzer 只冻结 `Input | Generated` 结构；preflight 才解释 task，且 VoiceClone reference audio 只进入独立 conditioning contract。          |

## 9. 非目标与参考

非目标包括 `/audio/*`、Responses audio、Realtime、未进入固定 profile 的 remote/multi-audio/格式、ASR 方言承诺、未单独验证的
VoiceDesign/VoiceClone 扩展格式与 voice identity/resource 复用。

- [OpenAI Chat 音频输入与输出调研](../../references/openai/audio/chat-input-output.md)
- [Xiaomi MiMo 全模型语音能力与调用途径](../../references/providers/xiaomi/audio.md)
