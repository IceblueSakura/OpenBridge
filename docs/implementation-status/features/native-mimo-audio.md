# 功能：MiMo 专用 ASR/TTS/VoiceDesign/VoiceClone Chat Native 接入

## 状态

**已完成（当前 checkout）。** Xiaomi MiMo 的四个专用语音模型已经进入 canonical Model、Upstream Target、Chat Native Route
和 Public Model 目录。它们都固定转发到 MiMo `/v1/chat/completions`；本轮没有把 `mimo-v2.5` 通用音频理解、OpenAI
`/v1/audio/*`、Responses audio 或 Realtime 声明为已实现。

## 已完成内容

- 注册 `mimo-v2.5-asr`、`mimo-v2.5-tts`、`mimo-v2.5-tts-voicedesign` 和
  `mimo-v2.5-tts-voiceclone` 四个独立 canonical Model 与 Public Model。
- 每个模型只有一个固定 Chat Native surface，不生成 Responses Route、Chat/Responses Bridge 或跨任务 fallback candidate；下游
  `model`、Chat body、顶层 `asr_options`/`modalities`/`audio` 和音频 part 保持原样转发。
- 四个音频 target 都把 Provider-wide function-tool ceiling 收窄为不支持；扩展 Models 不公开 `tools`、`tool_choice` 或
  `parallel_tool_calls`，带 function tool 的合法音频 task 在 Provider egress 前拒绝。
- 四个音频 target 同样把 structured outputs 收窄为不支持；`response_format` 不会与 ASR/TTS/VoiceDesign/VoiceClone 媒体任务组合
  转发，task-valid 请求在 Provider egress 前返回稳定能力错误。
- ASR profile 只接受一个 user `input_audio`，首批 source 为 data URL/pure Base64，format 为 WAV，language 为 `auto`/`zh`/`en`。
  混入 text、多音频、非 user 角色、Responses audio 或非法 Base64 在 Provider egress 前拒绝。
- TTS profile 接受一个 assistant 目标文本，可带 user 风格文本；JSON 输出 format 只开放 WAV，streaming Chat audio format 只开放
  PCM16，并公开 preset voice `mimo_default`。
- VoiceDesign 使用 user 文本作为 voice description，VoiceClone 使用独立 `audio.voice` data URL 作为
  `voice_conditioning`；两个 profile 都只开放 WAV JSON output、PCM16 streaming output，且不建立跨请求 voice identity 或资源复用。
- Models 扩展接口公开 `audio_task`、`multimodal_input.audio`、`multimodal_input.voice_conditioning` 和
  `multimodal_output.audio` 的 source、format、voice 与有界 encoded/decoded limits；请求分析只保留 bounded metadata，不保存或解码
  音频内容。
- Chat capability 只保存一个 typed `audio` profile；`multimodal.input/output` 的存在由 profile 中的 input、voice-conditioning 和
  output 子 profile 推导，不再由重复的 `audio_input`/`audio_output` 布尔字段声明。`AudioTask::Any` 仅可作为 Provider ceiling，不能成为
  可执行 Route 的 task identity。

## 实现边界

- canonical facts 位于 [`src/models/xiaomi/`](../../../src/models/xiaomi/)；MiMo Provider contract、任务 profile 和固定 target 位于
  [`src/providers/mimo/`](../../../src/providers/mimo/)。Public Model registration 位于
  [`src/providers/catalog/public_models.rs`](../../../src/providers/catalog/public_models.rs)。
- 音频能力类型和 subset 规则位于 [`src/core/capability/generation.rs`](../../../src/core/capability/generation.rs)；Models DTO 和
  多候选保守交集位于 [`src/registry/public_model.rs`](../../../src/registry/public_model.rs) 及其 compiler 子模块。
- Chat 音频 shape、source、format、Base64 与 URL 边界分析位于 [`src/pipeline/analysis/generation/audio.rs`](../../../src/pipeline/analysis/generation/audio.rs)，
  task-specific preflight 位于 [`src/pipeline/preflight.rs`](../../../src/pipeline/preflight.rs)。实现不下载、解码、转码、重采样、播放、落盘或缓存音频。
- 10 MB inline profile ceiling 仍受部署级 `max_request_body_bytes` 独立限制；默认 body 限制可能先于模型 profile 拒绝大请求。响应音频
  的内容语义、采样率、声道与真实媒体可播放性不由本轮 gateway 解析器证明。

## 验证证据

- `cargo test --locked --test example_config mimo_models_compile_model_specific_native_surfaces`：通过，覆盖四个模型的
  canonical facts、target、Public Model 和 Chat-only route surface。
- `cargo test --locked --test capability_definition_contract`：通过，覆盖 typed audio profile 的能力收窄和保留字段边界。
- `cargo test --locked --test forwarding_contract mimo_audio`：通过，覆盖四个模型的 JSON/SSE Chat wire 保真、Models typed contract、
  task-specific 形状拒绝、function-tool contract 和 zero-egress 失败边界。
- 2026-08-08 使用本地私有 credential 直连真实 MiMo Provider：ASR 返回语义正确的短 WAV transcript；TTS、VoiceDesign 与
  VoiceClone 均返回可在内存中解码的 RIFF/WAV。四模型携带 `tool_choice: "required"` 的对照请求均返回 `tool_calls: null` 并继续
  原音频任务，因此不构成工具调用支持。完整脱敏矩阵见 [MiMo Provider 状态](../providers/mimo.md)。
- 2026-08-09 真实交叉结果确认四个音频模型的 `json_object/json_schema` × JSON/SSE 为 0/16；收窄后 task-valid 同维度 HTTP
  矩阵 16/16 在本地返回 `unsupported_model_capability`。`mimo_unreliable_tool_and_structured_output_combinations_fail_before_egress`
  证明这些请求不进入 transport。
- 最终 Rust 基线：`cargo fmt -- --check`、`cargo test --locked`（64 个单元测试及全部契约测试通过）、
  `cargo clippy --locked -- -D warnings` 和 `git diff --check` 均通过；本专题不把静态/loopback 测试写成真实 Provider 验收。

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
