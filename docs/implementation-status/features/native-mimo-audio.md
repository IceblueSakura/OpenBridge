# 功能：MiMo 专用 ASR/TTS/VoiceDesign/VoiceClone Chat Native 接入

## 状态

**已完成（当前 checkout）。** Xiaomi MiMo 的四个专用语音模型已经进入 canonical Model、Upstream Target、Chat Native Route
和 Public Model 目录。它们都固定转发到 MiMo `/v1/chat/completions`；本轮没有把 `mimo-v2.5` 通用音频理解、OpenAI
`/v1/audio/*`、Responses audio 或 Realtime 声明为已实现。

## 已完成内容

- 注册 `mimo-v2.5-asr`、`mimo-v2.5-tts`、`mimo-v2.5-tts-voicedesign` 和
  `mimo-v2.5-tts-voiceclone` 四个独立 canonical Model 与 Public Model；其 mandatory canonical task 分别为
  `SpeechRecognition`、`SpeechSynthesis`、`VoiceDesign`、`VoiceClone`，运行实体只保存对应 task payload。
- 每个模型只有一个固定 Chat Native surface，不生成 Responses Route、Chat/Responses Bridge 或跨任务 fallback candidate；下游
  `model`、Chat body、顶层 `asr_options`/`modalities`/`audio` 和音频 part 保持原样转发。
- 四个音频 target 都把 Provider-wide function-tool ceiling 收窄为不支持；扩展 Models 不公开 `tools`、`tool_choice` 或
  `parallel_tool_calls`，带 function tool 的合法音频 task 在 Provider egress 前拒绝。
- 四个音频 target 同样把 structured outputs 收窄为不支持；`response_format` 不会与 ASR/TTS/VoiceDesign/VoiceClone 媒体任务组合
  转发，task-valid 请求在 Provider egress 前返回稳定能力错误。
- ASR profile 只接受一个 user `input_audio`，首批 source 为 data URL/pure Base64，format 为 WAV，language 为 `auto`/`zh`/`en`。
  混入 text、多音频、非 user 角色、Responses audio 或非法 Base64 在 Provider egress 前拒绝。
- TTS profile 接受一个 assistant 目标文本，可带 user 风格文本；JSON 输出 format 只开放 WAV，streaming Chat audio format 只开放
  PCM16，并公开 preset voice `mimo_default`。下游 `audio.voice` 保持可省略；显式提供 preset 时只接受 `mimo_default`，adapter
  不为省略值注入另一个 wire 字段。
- VoiceDesign 使用 user 文本作为 voice description，VoiceClone 使用独立 `audio.voice` data URL 作为
  `voice_conditioning`；两个 profile 都只开放 WAV JSON output、PCM16 streaming output，且不建立跨请求 voice identity 或资源复用。
- Models 扩展接口公开 `audio_task`、`multimodal_input.audio`、`multimodal_input.voice_conditioning` 和
  `multimodal_output.audio` 的 source、format、voice 与有界 encoded/decoded limits；请求分析只保留 bounded metadata，不保存或解码
  音频内容。四个 Public Model 的 `capabilities.tasks` 分别只公开 `speech_recognition`、`speech_synthesis`、`voice_design`、
  `voice_clone`；private union tag 不进入 Models JSON。
- MiMo Provider audio ceiling 是非空、task 不重复的五种完整 profile 集合；每个专用 Target 的 Chat capability 只保存一个同名
  executable profile。ceiling 与 executable profile 使用不同静态类型；`multimodal.input/output` 由 concrete variant 的 input、
  voice-conditioning、output 派生。
- 三个 generated-audio Target 的 profile 都同时携带必填 JSON WAV delivery 与 SSE PCM16 delivery；两种 delivery 均有非空 format、
  正数 budget 和固定 framing。普通 TTS 独有 preset voice payload；VoiceDesign/VoiceClone 不使用空 voice 集合作为哨兵，VoiceClone
  另有必填 conditioning profile。
- Registry 在构造 snapshot 前先做 Provider ceiling containment，再做 canonical task/executable profile compatibility；专用 task
  缺 profile、绑定其他 variant 或越过 ceiling 都返回 typed startup error。Public Model 编译还拒绝跨 operation task 混合，以及同
  task/same variant 的空 payload 交集。

## 实现边界

- canonical facts 位于 [`src/models/xiaomi/`](../../../src/models/xiaomi/)；MiMo Provider contract、任务 profile 和固定 target 位于
  [`src/providers/mimo/`](../../../src/providers/mimo/)。Public Model registration 位于
  [`src/providers/catalog/public_models.rs`](../../../src/providers/catalog/public_models.rs)。
- 音频能力类型和 subset 规则位于 [`src/core/capability/generation.rs`](../../../src/core/capability/generation.rs)；Models DTO 和
  多候选保守交集位于 [`src/registry/public_model.rs`](../../../src/registry/public_model.rs) 及其 compiler 子模块。
- Chat 音频 shape、source、format、Base64 与 URL 边界分析位于 [`src/pipeline/analysis/generation/audio.rs`](../../../src/pipeline/analysis/generation/audio.rs)，
  task-specific preflight 位于 [`src/pipeline/preflight.rs`](../../../src/pipeline/preflight.rs)。Analyzer 只构造
  `RequestedAudio::Input | Generated` task-neutral structural union，保存 resources/delivery、
  `InputAudioMessageShape`/`GeneratedAudioMessageShape`、ASR option presence 和 unspecified/preset/reference voice shape；只有 preflight
  取得已编译 interface 后才解释 ASR/TTS/VoiceDesign/VoiceClone。ASR 只接受单个 user audio message，VoiceClone 只接受单个 assistant
  text message，TTS 接受单个 assistant text 或 user text 后接 assistant text，VoiceDesign 只接受后一组合；其他、额外、空或角色错误
  message 均 fail closed。
  VoiceClone reference audio 只进入 `voice_conditioning`。实现不下载、解码、转码、重采样、播放、落盘或缓存音频。
- 10 MB inline profile ceiling 仍受部署级 `max_request_body_bytes` 独立限制；默认 body 限制可能先于模型 profile 拒绝大请求。响应音频
  的内容语义、采样率、声道与真实媒体可播放性不由本轮 gateway 解析器证明。

## 验证证据

- `cargo test --locked --test example_config`：通过；其中 canonical catalog/task 表、audio task/profile mismatch、Provider ceiling miss、
  AudioUnderstanding modality matrix 和 Public Model profile 交集测试覆盖启动边界。
- 完整 `cargo test --locked` 中的 `core::capability::generation::tests` 通过，覆盖五种完整 audio profile、Provider ceiling same-variant
  containment 和 checked input/delivery 构造边界。
- 完整 Rust baseline 中的 `forwarding_contract` 通过；`mimo_audio_models_are_chat_native_and_keep_task_specific_wire`、
  `mimo_audio_task_mismatches_fail_before_egress` 等用例覆盖四个模型的 JSON/SSE Chat wire 保真、canonical task Models 投影、private
  union tag 不泄漏、VoiceClone conditioning、task-specific shape、function-tool contract 和 zero-egress 失败边界。
- 2026-08-08 使用本地私有 credential 直连真实 MiMo Provider：ASR 返回语义正确的短 WAV transcript；TTS、VoiceDesign 与
  VoiceClone 均返回可在内存中解码的 RIFF/WAV。四模型携带 `tool_choice: "required"` 的对照请求均返回 `tool_calls: null` 并继续
  原音频任务，因此不构成工具调用支持。完整脱敏矩阵见 [MiMo Provider 状态](../providers/mimo.md)。
- 2026-08-09 真实交叉结果确认四个音频模型的 `json_object/json_schema` × JSON/SSE 为 0/16；收窄后 task-valid 同维度 HTTP
  矩阵 16/16 在本地返回 `unsupported_model_capability`。`mimo_unreliable_tool_and_structured_output_combinations_fail_before_egress`
  证明这些请求不进入 transport。
- 当前 Rust 基线实际运行 `cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings` 和
  `git diff --check`，均通过；本专题不把静态/loopback 测试写成真实 Provider 验收。

## 未覆盖范围

- `mimo-v2.5` 通用音频理解；OpenAI `/v1/audio/speech`、`/v1/audio/transcriptions`、`/v1/audio/translations`、Responses audio、Realtime。
- 经 OpenBridge 下游接口完成的四模型真实端到端复测；本次新增证据是直连 Provider，不能替代网关路径验收。
- ASR MP3/更广语言和方言、TTS MP3/其他 voice、VoiceDesign/VoiceClone 的扩展格式、duration/sample-rate/channel 校验。
- voice authorization、保留期、跨请求 voice identity/resource store、媒体下载/解码/转码、播放和持久化。
- OpenAI SDK、目标 Agent、负载、长期运行以及硬件/播放器验收。

## 相关文档

- [功能需求：Native 音频能力](../../functional-requirements/native-audio.md)
- [MiMo 全模型语音能力与调用途径](../../references/providers/xiaomi/audio.md)
- [MiMo Provider 多模态与工具调用状态](../providers/mimo.md)
- [标准 Audio/Speech 协议索引](../../references/openai/README.md#6-音频与语音)
- [Chat/Responses Native 转发](native-generation-forwarding.md)
- [Models 接口与能力预检](models-api-and-capability-preflight.md)
