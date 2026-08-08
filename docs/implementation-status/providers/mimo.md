# Xiaomi MiMo Provider 多模态与工具调用状态

## 状态

**当前已注册，且完成一次真实 Provider 能力探测。** 当前 checkout 为 `mimo-primary` 注册六个 Public Model。2026-08-08 使用
本地私有 credential 直连 `https://api.xiaomimimo.com`，`GET /v1/models` 返回 HTTP 200，并列出下表六个 model ID。
真实请求仅用于确认 Provider 行为；除已有图片验收外，本页不把直连结果写成 OpenBridge 下游到上游端到端验收。

## 多模态支持矩阵

“实测”表示本次真实 Provider 请求得到了与任务相符的结果；“当前实现”表示 OpenBridge 当前 Public Model interface 和确定性测试
覆盖的范围。

| Public Model | 当前接口 | 文本输入 | 图片输入 | 音频输入或条件 | 音频输出 | 视频输入 | 当前实现与实测结论 |
|---|---|---|---|---|---|---|---|
| `mimo-v2.5-pro` | Chat、Responses；Native 与受限 Bridge | 实测支持；Chat 返回文本，Chat/Responses 工具请求均完成 | 未声明 | 未声明 | 未声明 | 未声明 | OpenBridge 当前按 text-only 编译；本次没有把未声明模态当作真实负向探测 |
| `mimo-v2.5` | Chat、Responses Native | 实测支持；两协议均返回可见文本 | 实测支持；64×64 PNG data URL 在两协议均正确识别主色 | Provider 实测支持通用音频理解；短 WAV 被正确理解 | 未声明 | 模型目录声明，未实测 | OpenBridge 当前只实现 text/image；通用音频理解和 video 尚未进入可执行 interface |
| `mimo-v2.5-asr` | Chat Native | 不接受普通文本输入；输出 transcript | 未声明 | 实测支持；单个 WAV + `asr_options` 返回正确 transcript | 未声明 | 未声明 | OpenBridge 已实现单 WAV ASR task profile；真实请求 HTTP 200 |
| `mimo-v2.5-tts` | Chat Native | 实测支持目标文本与风格文本 | 未声明 | 不接收业务音频输入 | 实测支持；返回可解码 RIFF/WAV | 未声明 | OpenBridge 已实现 preset voice TTS；本次实测非流式 WAV，streaming PCM16 仍只有确定性证据 |
| `mimo-v2.5-tts-voicedesign` | Chat Native | 实测支持音色描述与目标文本 | 未声明 | 不接收 reference audio | 实测支持；返回可解码 RIFF/WAV | 未声明 | OpenBridge 已实现 VoiceDesign task profile；真实请求 HTTP 200 |
| `mimo-v2.5-tts-voiceclone` | Chat Native | 实测支持目标文本 | 未声明 | 实测支持 reference WAV voice conditioning；不是音频理解或 ASR | 实测支持；返回可解码 RIFF/WAV | 未声明 | OpenBridge 已实现 VoiceClone task profile；真实请求 HTTP 200 |

`mimo-v2.5` 的通用音频理解与 `mimo-v2.5-asr` 的专用转写不是同一能力；VoiceClone 的 reference audio 也只是音色条件，不能视为
音频理解。当前 MiMo 音频正向请求全部使用 `POST /v1/chat/completions`。同一 origin 的 OpenAI 标准
`POST /v1/audio/speech` 和 `POST /v1/audio/transcriptions` 对照请求均返回 HTTP 404、`text/html`。

## 工具调用支持矩阵

真实正向探测使用一个无副作用的 `report_result` function，设置 `tool_choice: "required"`，只检查是否返回有效函数名和调用结构；
OpenBridge 不执行该工具。

| Public Model | 真实 Provider 结果 | 当前确定性证据 | 结论 |
|---|---|---|---|
| `mimo-v2.5-pro` | Chat 返回 `finish_reason: "tool_calls"` 和 1 个有效 function call；Responses 返回 1 个 `function_call` output item | `mimo_models_compile_model_specific_native_and_bridge_surfaces` 覆盖 Chat/Responses Native 与 Bridge candidate 的 function-tool 规划 | Chat、Responses 工具调用实测支持；真实 Bridge 端到端未验证 |
| `mimo-v2.5` | Chat 与 Responses 各返回 1 个有效 function call | 同一编译测试覆盖两协议 Native 规划；`mimo_responses_native_preserves_parallel_tool_stream` 覆盖 Responses streaming 并行调用保真 | Chat、Responses 工具调用实测支持；真实并行调用未验证 |
| `mimo-v2.5-asr` | HTTP 200，但 `tool_calls: null`、`finish_reason: "stop"`，仍返回 transcript | canonical `supported_parameters` 不含 `tools`；音频 task wire 有确定性转发测试 | 不支持；Provider 静默忽略工具字段 |
| `mimo-v2.5-tts` | HTTP 200，但 `tool_calls: null`、`finish_reason: "stop"`，仍返回 audio | canonical `supported_parameters` 不含 `tools`；TTS wire 有确定性转发测试 | 不支持；Provider 静默忽略工具字段 |
| `mimo-v2.5-tts-voicedesign` | HTTP 200，但 `tool_calls: null`、`finish_reason: "stop"`，仍返回 audio | canonical `supported_parameters` 不含 `tools`；VoiceDesign wire 有确定性转发测试 | 不支持；Provider 静默忽略工具字段 |
| `mimo-v2.5-tts-voiceclone` | HTTP 200，但 `tool_calls: null`、`finish_reason: "stop"`，仍返回 audio | canonical `supported_parameters` 不含 `tools`；VoiceClone wire 有确定性转发测试 | 不支持；Provider 静默忽略工具字段 |

## 当前实现偏差

四个音频专用模型的 canonical `supported_parameters` 均没有 `tools` 或 `tool_choice`，真实 Provider 也没有产生有效 tool call；但当前
MiMo audio target 仍继承 Provider-wide `function_tools` 上界，因此扩展 Models interface 可能过宽公开工具能力，并允许带工具字段的合法
音频 task 到达上游。这是当前 checkout 的能力收窄缺口。客户端在该缺口修复前不应向四个音频专用模型发送工具字段，也不能把 HTTP 200
和被忽略的 `tool_choice: "required"` 当作支持。

该偏差不应通过按能力选择 Route 或能力 fallback 处理；每个音频 target 应形成与自身模型和任务一致的固定 interface。同一 Public Model
仍只允许在其固定 Provider candidates 之间按现有顺序 fallback。

## 验证证据

2026-08-08 真实 Provider 探测：

- `GET /v1/models`：HTTP 200，返回六个当前 model ID；
- `mimo-v2.5`：Chat/Responses 文本与图片均 HTTP 200；Chat 通用音频理解 HTTP 200；
- `mimo-v2.5-pro`：Chat 文本以及 Chat/Responses function tool 均 HTTP 200；
- ASR：短合成 WAV 返回非空且语义正确的 transcript；
- TTS、VoiceDesign、VoiceClone：均返回 Base64，可在内存中解码为 RIFF/WAV；
- 六模型工具探测：两个通用模型产生有效 function call，四个音频专用模型返回 `tool_calls: null` 并继续原任务；
- `/v1/audio/speech`、`/v1/audio/transcriptions`：均 HTTP 404。

当前 checkout 的确定性证据入口：

- [`tests/example_config.rs`](../../../tests/example_config.rs)：MiMo model、target、Route、图片和工具规划；
- [`tests/forwarding_contract.rs`](../../../tests/forwarding_contract.rs)：图片/音频 wire、任务拒绝和 Responses 并行工具流；
- [`tests/provider_boundary_contract.rs`](../../../tests/provider_boundary_contract.rs)：MiMo Provider 能力上界；
- [`tests/capability_definition_contract.rs`](../../../tests/capability_definition_contract.rs)：typed 多模态/工具能力定义与收窄规则。

所有真实请求只使用合成文本、内存 PNG 和内存 WAV；没有记录 credential、完整请求/响应、原始 Base64、完整 reasoning、Provider
request ID 或音频文件。

## 未覆盖范围

- `mimo-v2.5` video、remote audio、多个 audio part、其他图片/音频格式和上限；
- 真实 parallel tool calls、strict schema、全部 `tool_choice` mode、tool-result round trip；
- 四个音频模型的 streaming 真实 Provider、OpenAI SDK、目标 Agent、负载和长期运行；
- ASR 人声/方言质量、TTS 音色质量与播放器验收；
- 除已有图片路径外，经临时 OpenBridge 实例完成的六模型下游到上游端到端复测。

## 相关文档

- [Provider 状态目录](README.md)
- [MiMo 专用音频实现](../features/native-mimo-audio.md)
- [`mimo-v2.5` Native 图片输入](../features/native-image-input.md)
- [Models 接口与能力预检](../features/models-api-and-capability-preflight.md)
- [MiMo 外部音频协议与能力参考](../../references/providers/xiaomi/xiaomi-mimo-audio-capabilities-2026-08-08.md)
