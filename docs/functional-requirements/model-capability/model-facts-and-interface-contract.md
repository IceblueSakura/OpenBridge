# 模型事实与固定接口契约

## 状态

本文是[模型与能力契约域](README.md)的契约模块：定义 `PublicModelInfo` 公共对象、未知语义与固定契约的保守相交计算。
其他模块见[模型与能力契约域](README.md)导航。

## 1. 公共对象

`PublicModelInfo` 必须包含：

- OpenAI 标准身份：`id`、`object`、`created`、`owned_by`；
- 生命周期和展示信息：`name`、`description`、`lifecycle`；
- 模型事实：任务、total/input/output context、输入/输出模态、tokenizer、知识截止和 reasoning；
- 接口契约：`chat_completions` 与 `responses` 各自至多一个生成接口能力对象，分别公开 `streaming` 与 `non_streaming` 支持状态，并可带协议内 source-aware `multimodal_input`；固定
  音频生成任务还可带 mode-aware `multimodal_output.audio`；生成接口还公开逐值 `response_includes`，Chat 或没有共同可接受 Responses
  include 值时为空数组；
  `embeddings` 至多一个独立 Embedding 接口能力对象；
- schema 版本固定为字符串 `"1"`；不提供 v2 alias、legacy 字段镜像或双写兼容层。

模型事实是模型本体的安全公共上界；模型请求是否可调用某能力，必须以目标 `interfaces` 项为准。某协议没有 可执行 Route 时，其接口值为
`null`。canonical Model 的参数事实只参与编译各接口的
`supported_parameters`，模型事实层不得再公开一份不能直接用于请求放行的重复列表。该字段表示 OpenBridge 接受对应顶层参数；对
[普通参数上游兼容规则](../gateway-api/parameter-compatibility.md)显式列出的字段，具体候选可以在 egress 前忽略，因而不承诺
每个 Provider 都会实际应用该提示。

`capabilities.tasks` 必须从唯一 canonical task 固定映射，不得从 Route operation、audio presence 或请求字段猜测：

| Canonical task       | Public task projection              |
|----------------------|-------------------------------------|
| `Generation`         | `chat`、`text_generation`           |
| `Embedding`          | `embedding`                         |
| `SpeechRecognition`  | `speech_recognition`                |
| `SpeechSynthesis`    | `speech_synthesis`                  |
| `VoiceDesign`        | `voice_design`                      |
| `VoiceClone`         | `voice_clone`                       |

Generation reasoning 只由 `Unsupported | Unknown | Supported { levels }` 保存，levels 是有序、唯一的 checked set。普通参数不得保存
`reasoning` 或 `reasoning_effort` sentinel；Public Model compiler 必须按 canonical reasoning profile 与目标 downstream protocol
派生对应 wire parameter。

## 2. 未知语义

- 布尔能力使用 `supported`、`unsupported`、`unknown`；只有 `supported` 能通过请求预检。
- 未知 token 限制、tokenizer、知识截止或日期使用 JSON `null`。
- 数组只包含已确认值，必须去重并确定性排序；空数组表示没有可公开保证的值。
- `unknown` 不能按"上游也许支持"提升为 `supported`；`unsupported` 不能伪装成 `unknown`。

OpenRouter canonical model 的 `context_length` 是模型目录公开的上下文上限，而不是独立的
`max_input_tokens` 字段。OpenBridge 将这项已确认的模型级上限投影到 `max_context_tokens` 和
`max_input_tokens`；`top_provider.max_completion_tokens` 只用于 `max_output_tokens`。不把总上下文减去 最大输出做未经
OpenRouter 声明的残差推导；若某个具体 Upstream API 更窄，应通过
`UpstreamApiModelRules` 明确收窄。

## 3. 固定契约计算

每个 Public Model 对 Chat Completions、Responses 和 Embeddings 分别只有一个固定契约。registry 在启动时把该 operation 全部
静态启用、可执行的 Route 作为契约输入，并按以下规则保守相交：

| 字段                                                                                | 计算规则                                                                                                              |
|-------------------------------------------------------------------------------------|-----------------------------------------------------------------------------------------------------------------------|
| 布尔能力                                                                            | 全部 Route 明确支持才是 `supported`；任一明确不支持则为 `unsupported`；证据不足保持 `unknown`                         |
| token 上限                                                                          | 全部 Route 都有已知值时取最小值；任一未知则为 `null`                                                                  |
| 模态、参数、实际可执行 reasoning level                                             | 取集合交集并稳定排序；某 API 的普通忽略参数仍属于其下游可接受参数                                                     |
| image/file/audio source、inline encoding、format、detail allowed、可验证 media type | 按目标协议分别取集合交集；detail default 必须一致，任一必需集合为空则对应媒体输入子契约不可公开                       |
| audio output mode、format、voice、encoding/container、采样参数与上限                 | 按 JSON/SSE mode 分别保守相交；条件 format 不得压平，任一 mode 无完整 framing/累计预算时不得公开                      |
| media part、URL 长度、inline 编码/解码字节上限                                      | 取全部 Route 保证值与 gateway hard limit 中的最小值；累计字节只统计 inline payload                                    |
| reasoning 输出形态                                                                  | 全部 Route 形态相同时公开该值，否则为 `unknown`                                                                       |
| Responses `include` 条件输出请求                                                    | Route contribution 携带逐值 public accepted 集合，Public Model 取全部固定候选交集；candidate forwarded set 保持私有；公开值不保证对应 item 存在，唯一 approved omitted-equivalent hint 可在 Native/Bridge candidate planning 中逐值删除 |
| function tools                                                                  | `type`、`tool_choice` mode、parallel calls 与 strict schema 分字段声明；每个集合取所有 Route 的交集，不得因 `support: supported` 自动补齐 mode；parallel 只承诺接受请求值，不保证调用数量或执行并发 |
| structured outputs                                                              | 执行契约只保存 `JsonObject | JsonSchema(strictness) | JsonObjectAndJsonSchema(strictness)` 闭合 profile；按完整 variant 相交，空 mode 交集关闭整个能力，Models 的 support/modes/strict 只从结果投影 |
| `Bridged` Route                                                                     | 只贡献转换器完整支持的公共子集；image/file/audio source 与 audio output 贡献空集                                     |

reasoning 的 `levels` 始终表示上述可执行交集。generation Public Model 另以静态 `input_policy` 决定下游输入：`strict`
只接受 `levels`；`clamp_positive_floor` 在正向序列中向下取不高于请求值的最高档，低于最小档时夹到最小档，并由此投影
`accepted_levels`。`none` 保持独立，只有它属于实际 `levels` 时才进入 `accepted_levels`，不能被正向策略吸收。

Embedding 接口不使用生成协议的 token-output、tool、reasoning 或 stream 字段。它应独立保守相交 input forms、默认/可显式请求的
output encoding、默认维度、可请求 dimension domain 和输入/批量限制；encoding 与 dimensions 都不得压缩成布尔值。公开
`max_inputs` 还必须被 gateway batch/response budget 收窄，不能接受一个必然产生本地超限成功体的请求。不同 vector identity
未被显式证明等价时，不得编译进同一可 fallback 契约。Embedding Route 只允许 Native，不从 Chat/Responses Bridge 派生。

Chat/Responses 的 `modalities.input`/`modalities.output` 只是摘要。具体 image/file/audio 请求还必须匹配 `multimodal_input` 中的协议
part、source、inline encoding、format/detail/media type 与 limits；音频生成还必须匹配 `multimodal_output.audio` 的 JSON/SSE mode、
format、voice、framing 与累计预算。嵌套 content part 字段不加入顶层 `supported_parameters`；task-specific `asr_options`/`audio` 只在
对应 interface 顶层公开。共同编译规则见[扩展导航](../extended-capabilities/README.md)，闭合集合分别由
[图片](../extended-capabilities/native-image.md)、[文件](../extended-capabilities/native-file.md)和[音频](../extended-capabilities/native-audio.md)功能页拥有。

音频输入还必须绑定业务用途：`content_understanding`、`speech_recognition` 与 `voice_conditioning` 不能因为都使用 Base64/URL 或
`input_audio` 而聚合。模型任务、输入用途、输出语义和 typed parameter 任一不同，都必须使用独立 interface/Public Model contract。
同一 Public Model 的全部 Route 必须先证明 canonical task 相同；同一 operation 的 audio candidate 还必须是同一个 executable profile
variant，且完整 payload 交集非空。VoiceClone 的 reference audio 只能投影到独立 `voice_conditioning`，不得伪装成 content-understanding
audio input。

能力不得按字段求并集，也不返回 `guaranteed + profiles`、conditional capability 或按 Route 展开的公共视图。
`previous_response_id` 只能由 executable `TargetBoundContinuation` profile 贡献。Route contribution 必须携带 issuer 的判别联合；
全部 Responses Route 明确支持且唯一解析到同一个 Upstream Target/API 后，Private execution interface 才保存
`Supported { issuer }`，Public JSON 仅投影 `SupportState` 与 parameter。存在多个潜在签发者或 Bridge 时必须公开为
`unsupported`，并从接口 `supported_parameters` 删除。

若同一 canonical Model 由多个 Provider Target 提供，只有代码目录将对应 route source 显式列入同一 Public Model 时才形成聚合；模型
ID 相同不能自动新增候选。聚合后每个协议的全部静态可执行 Route 仍共同参与上述 保守交集，不能只按首选 Provider 计算公共契约。

## 关联文档

- [模型与能力契约域导航](README.md)
- [事实所有权与公开边界](fact-ownership-and-boundary.md)
- [请求预检与禁止能力路由](request-preflight-and-routing.md)
- [扩展能力导航及共同规则](../extended-capabilities/README.md)
- [实施现状](../../implementation-status/README.md)
