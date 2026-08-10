# Xiaomi MiMo Provider 多模态与工具调用状态

## 状态

**当前已注册，且完成真实 Provider 与文本 high 端到端验收。** 当前 checkout 为 `mimo-primary` 注册六个 Public Model。2026-08-08 使用
本地私有 credential 直连 `https://api.xiaomimimo.com`，`GET /v1/models` 返回 HTTP 200，并列出下表六个 model ID。
另通过当前 OpenBridge 和真实下游用户 key 验证两个文本模型的 high reasoning；直连结果与端到端结果分别记录。

## 多模态支持矩阵

“实测”表示本次真实 Provider 请求得到了与任务相符的结果；“当前实现”表示 OpenBridge 当前 Public Model interface 和确定性测试
覆盖的范围。

| Public Model | 当前接口 | 文本输入 | 图片输入 | 音频输入或条件 | 音频输出 | 视频输入 | 当前实现与实测结论 |
|---|---|---|---|---|---|---|---|
| `mimo-v2.5-pro` | Chat、Responses Native | 实测支持；Chat/Responses high JSON/SSE 均完成并返回明文 reasoning | 未声明 | 未声明 | 未声明 | 未声明 | OpenBridge 按 text-only 编译，两接口公开 none/low/medium/high；未把其他模态外推到 Pro |
| `mimo-v2.5` | Chat、Responses Native | 实测支持；Chat/Responses high JSON/SSE 均完成并返回明文 reasoning | 实测支持；64×64 PNG data URL 在两协议均正确识别主色 | Provider 实测支持通用音频理解；短 WAV 被正确理解 | 未声明 | 模型目录声明，未实测 | OpenBridge 当前实现 text/image 和统一四档；通用音频理解和 video 尚未进入可执行 interface |
| `mimo-v2.5-asr` | Chat Native | 不接受普通文本输入；输出 transcript | 未声明 | 实测支持；单个 WAV + `asr_options` 返回正确 transcript | 未声明 | 未声明 | OpenBridge 已实现单 WAV ASR task profile；真实请求 HTTP 200 |
| `mimo-v2.5-tts` | Chat Native | 实测支持目标文本与风格文本 | 未声明 | 不接收业务音频输入 | 实测支持；返回可解码 RIFF/WAV | 未声明 | OpenBridge 已实现 preset voice TTS；本次实测非流式 WAV，streaming PCM16 仍只有确定性证据 |
| `mimo-v2.5-tts-voicedesign` | Chat Native | 实测支持音色描述与目标文本 | 未声明 | 不接收 reference audio | 实测支持；返回可解码 RIFF/WAV | 未声明 | OpenBridge 已实现 VoiceDesign task profile；真实请求 HTTP 200 |
| `mimo-v2.5-tts-voiceclone` | Chat Native | 实测支持目标文本 | 未声明 | 实测支持 reference WAV voice conditioning；不是音频理解或 ASR | 实测支持；返回可解码 RIFF/WAV | 未声明 | OpenBridge 已实现 VoiceClone task profile；真实请求 HTTP 200 |

`mimo-v2.5` 的通用音频理解与 `mimo-v2.5-asr` 的专用转写不是同一能力；VoiceClone 的 reference audio 也只是音色条件，不能视为
音频理解。当前 MiMo 音频正向请求全部使用 `POST /v1/chat/completions`。同一 origin 的 OpenAI 标准
`POST /v1/audio/speech` 和 `POST /v1/audio/transcriptions` 对照请求均返回 HTTP 404、`text/html`。

## Reasoning levels

- `mimo-v2.5` 与 `mimo-v2.5-pro` 的 canonical levels 都是 `none/low/medium/high`，Chat/Responses interface 公开同一集合；
  两个 Native API 的 reasoning output 为 `PlainText`，两个 Public Model 都不生成跨协议 Bridge。
- Chat egress 将 `none` 转为 `thinking.type=disabled`，其余三档转为 `enabled`；Responses 原样传递具体 effort。MiMo 官方当前
  说明三个开启档位行为相同，OpenBridge 仍保留各值以兼容后续差异化支持。
- 真实下游 E2E 覆盖两个模型的 Chat/Responses × JSON/SSE × high 共 8 个单元：全部 HTTP 200，JSON/SSE 终态完整且
  reasoning 非空。
- 四个 ASR/TTS target 继续把 reasoning output 收窄为 `Unknown`，不继承文本 target 的证据或 high level。
- 本轮请求只使用标准 Chat `reasoning_effort: "high"` 和 Responses `reasoning.effort: "high"`，不加载 Hermes、
  不调用 Hermes runtime，也不发送 Hermes custom 字段。

## 普通参数兼容

- 真实首选路径确认两个文本模型的 Chat `logprobs/top_logprobs` 均可直接透传；MiniMax、DeepSeek、GLM、Qwen 的已验证结果不会影响
  MiMo 自己的 API 规则。
- MiMo V2.5 与 Pro 的 Responses 对 `top_logprobs` 返回 `responses_feature_not_supported`。该字段会改变可观察输出结构，因此当前两个
  Responses Upstream API 将其显式禁用，Responses interface 不再公开并在 egress 前拒绝；Chat 同名字段继续透传，其他普通字段不受影响。
- 2026-08-09 既有真实结果证明两个模型 Chat 的 `logprobs/top_logprobs` 可透传，并证明 Responses 上游拒绝 `top_logprobs`；当前
  zero-egress 行为由 `tests/forwarding_contract.rs` 对两个 production model 的 HTTP code、精确 `param` 和空 transport 记录确定性覆盖。

## 工具调用支持矩阵

真实正向探测使用无副作用的 synthetic function，只检查终态、函数名、调用结构和 arguments schema；OpenBridge 不执行工具。

| Public Model | 真实 Provider 结果 | 当前确定性证据 | 结论 |
|---|---|---|---|
| `mimo-v2.5-pro` | Chat 返回 `finish_reason: "tool_calls"` 和 1 个有效 function call；Responses 返回 1 个 `function_call` output item | `mimo_models_compile_model_specific_native_surfaces` 覆盖 Chat/Responses Native function-tool 规划 | Chat、Responses 工具调用实测支持；parallel control 未验证，保持 unsupported |
| `mimo-v2.5` | Chat 与 Responses 各返回 1 个有效 function call；另行确认接受 `parallel_tool_calls:true` | 同一编译测试覆盖两协议 Native 规划；`mimo_responses_native_preserves_parallel_tool_control_and_multi_tool_stream` 覆盖 Responses exact egress 与 streaming 多调用保真 | Chat、Responses 工具调用及 parallel 请求参数受支持；不保证多调用或内部并发 |
| `mimo-v2.5-asr` | HTTP 200，但 `tool_calls: null`、`finish_reason: "stop"`，仍返回 transcript | canonical 参数和 Chat target 均不声明 tools；扩展 Models 公开 `unsupported`，带工具请求在 egress 前拒绝 | 不支持；Provider 静默忽略，OpenBridge fail closed |
| `mimo-v2.5-tts` | HTTP 200，但 `tool_calls: null`、`finish_reason: "stop"`，仍返回 audio | canonical 参数和 Chat target 均不声明 tools；扩展 Models 公开 `unsupported`，带工具请求在 egress 前拒绝 | 不支持；Provider 静默忽略，OpenBridge fail closed |
| `mimo-v2.5-tts-voicedesign` | HTTP 200，但 `tool_calls: null`、`finish_reason: "stop"`，仍返回 audio | canonical 参数和 Chat target 均不声明 tools；扩展 Models 公开 `unsupported`，带工具请求在 egress 前拒绝 | 不支持；Provider 静默忽略，OpenBridge fail closed |
| `mimo-v2.5-tts-voiceclone` | HTTP 200，但 `tool_calls: null`、`finish_reason: "stop"`，仍返回 audio | canonical 参数和 Chat target 均不声明 tools；扩展 Models 公开 `unsupported`，带工具请求在 egress 前拒绝 | 不支持；Provider 静默忽略，OpenBridge fail closed |

两个文字模型的 Chat/Responses 均只公开 `tool_choice:auto` 与 `strict_schema:supported`；只有 `mimo-v2.5` 按 2026-08-10 的直接接受
证据公开 `parallel_tool_calls`，Pro 保持 unsupported。官方说明 `none/required/named` 会被移除并退化成 auto，因此这些值即使返回
tool call 也不构成 choice 支持。Responses state machine 会保真自然或显式 parallel 请求产生的多个调用，但调用数量与执行并发仍由
上游决定。真实 auto + strict 的两模型、两协议和 JSON/SSE 共 8/8 返回合法 function call，arguments 严格匹配合成 schema。

## Structured outputs

当前固定能力按 model 和 operation 收窄：

| Public Model / interface | 当前公开模式 | 真实结论 |
|---|---|---|
| `mimo-v2.5` Chat、Responses | `json_object` | 按官方 JSON-only prompt 前提，JSON/SSE 四个组合均返回合法且字段匹配的 JSON |
| `mimo-v2.5-pro` Chat、Responses | `json_object` | 按相同前提，JSON/SSE 四个组合均返回合法且字段匹配的 JSON |
| 四个专用音频模型 Chat | 不支持 | 真实交叉矩阵 0/16；audio task 的有效媒体输出不等于结构化 text 输出 |

两款文字模型都不公开 `json_schema` 或 strict structured schema。JSON mode 只保证语法合法；OpenBridge 不重写 prompt、校验业务字段或
重试模型输出，也不因某个后备来源理论上更强而扩大固定 Public Model contract。

## 当前实现收窄

两个文本 target 将 function-tool choices 收窄为 `auto`、保留 strict function schema，并在 Chat/Responses 都只公开
`json_object`；`mimo-v2.5` 保留 Provider 的 parallel ceiling，Pro 在 target registration 层关闭。四个音频专用
模型的 canonical `supported_parameters` 均没有工具或结构化文本字段，真实 Provider 也没有产生有效
tool call/structured text。当前 model-specific audio target 已将 Chat `function_tools` 与 `structured_outputs` 收窄为 `None`；扩展
Models 因此公开两者 `unsupported`。合法音频 task 一旦携带这些能力，会在创建 RoutePlan 和 Provider egress 前返回 HTTP 400。

该收窄不通过按能力选择 Route 或能力 fallback 处理；每个音频 target 形成与自身模型和任务一致的固定 interface。同一 Public Model
仍只允许在其固定 Provider candidates 之间按现有顺序 fallback。

## 验证证据

2026-08-08 真实 Provider 探测：

- `GET /v1/models`：HTTP 200，返回六个当前 model ID；
- `mimo-v2.5`：Chat/Responses 文本与图片均 HTTP 200；Chat 通用音频理解 HTTP 200；
- `mimo-v2.5-pro`：Chat 文本以及 Chat/Responses function tool 均 HTTP 200；
- 两个文本模型的 Chat/Responses high JSON/SSE 共 8 个端到端单元全部 HTTP 200、终态完整且 reasoning 非空；
- ASR：短合成 WAV 返回非空且语义正确的 transcript；
- TTS、VoiceDesign、VoiceClone：均返回 Base64，可在内存中解码为 RIFF/WAV；
- 六模型工具探测：两个通用模型产生有效 function call，四个音频专用模型返回 `tool_calls: null` 并继续原任务；
- `/v1/audio/speech`、`/v1/audio/transcriptions`：均 HTTP 404。

本次工具能力收窄的 TDD 证据：聚焦 forwarding 测试在实现前同时观察到扩展 Models 错误公开 `supported` 和带工具 ASR 请求到达
transport；收窄 audio target 后，同一测试确认四模型公开 `unsupported`、三个工具参数均不出现，并对四种合法音频 task 保持 HTTP 400
与 zero egress。

当前 checkout 的确定性证据入口：

- [`tests/example_config.rs`](../../../tests/example_config.rs)：MiMo model、target、Route、图片和工具规划；
- [`tests/forwarding_contract.rs`](../../../tests/forwarding_contract.rs)：图片/音频 wire、任务拒绝和 Responses 并行工具流；
- [`tests/provider_boundary_contract.rs`](../../../tests/provider_boundary_contract.rs)：MiMo Provider 能力上界；
- [`tests/capability_definition_contract.rs`](../../../tests/capability_definition_contract.rs)：typed 多模态/工具能力定义与收窄规则。

2026-08-09 聚焦验证当时：structured-output 矩阵 8/8，auto + strict function-tool 矩阵 8/8，四个 MiMo tool-result continuation
接口 4/4。非 auto choice 24/24、当时尚未开放的 `parallel_tool_calls` 8/8、`json_schema` 8/8 均在本地返回 HTTP 400
`unsupported_model_capability`；确定性 forwarding 契约同时证明 zero egress。

2026-08-10 对 `mimo-v2.5` 直连 Chat 确认 `parallel_tool_calls:true` 返回 HTTP 200；当前 Models 与 Native Chat/Responses egress
据此开放并保留该值，Pro 仍 zero-egress fail closed。Responses 的带/不带 `reasoning.encrypted_content` 对照都返回相同明文
`reasoning_text`，因此该 include 只按接受兼容参数公开，不承诺 output item；MiMo V2.5 Native egress 原样转发，Pro 保持空 include 集合。

所有真实请求只使用合成文本、内存 PNG 和内存 WAV；没有记录 credential、完整请求/响应、原始 Base64、完整 reasoning、Provider
request ID 或音频文件。

## 未覆盖范围

- `mimo-v2.5` video、remote audio、多个 audio part、其他图片/音频格式和上限；
- 更多提示与长对话下的 auto/parallel 工具选择稳定性；HTTP 200 或多调用输出不证明上游内部并发执行；
- 四个音频模型的外部 OpenAI SDK、目标 Agent、负载和长期运行；
- 两个文本模型的 `none/low/medium` 真实 Chat/Responses JSON/SSE；
- ASR 人声/方言质量、TTS 音色质量与播放器验收；
- 四个专用音频模型经真实下游 key 的 task-specific JSON/SSE 端到端复测。

## 相关文档

- [Provider 状态目录](README.md)
- [MiMo 专用音频实现](../features/native-mimo-audio.md)
- [`mimo-v2.5` Native 图片输入](../features/native-image-input.md)
- [Models 接口与能力预检](../features/models-api-and-capability-preflight.md)
- [MiMo 外部音频协议与能力参考](../../references/providers/xiaomi/audio.md)
