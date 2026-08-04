# Audio Transcription/Translation 协议实现细节

**目标状态：** 仅作协议参考，不在现阶段 1/2 实施范围。

## 范围与 endpoint 差异

本协议族包含：

- `POST /v1/audio/transcriptions`：把音频转成原语言文本；当前 endpoint schema 接受 `multipart/form-data`，成功可为结构化转写对象或 transcript SSE；
- `POST /v1/audio/translations`：把音频翻译成英文；同样接受 multipart，但当前 endpoint schema 没有声明与 transcription 相同的 SSE 成功模式。

两个 endpoint 共享上传音频的外形，但 model、response format、timestamps、diarization、logprobs、language/prompt 和 stream 支持并不相同。实现必须使用两个 operation contract，不能只替换 path。

官方资料：[Create transcription](https://developers.openai.com/api/reference/resources/audio/subresources/transcriptions/methods/create)、[Create translation](https://developers.openai.com/api/reference/resources/audio/subresources/translations/methods/create)、[Audio and speech](https://developers.openai.com/api/docs/guides/audio) 与 [Speech to text](https://developers.openai.com/api/docs/guides/speech-to-text)。

## multipart ingress

现有业务 handler 只接受 JSON，不能承载此协议。multipart 实现必须在以下两种方式中明确选一种：

| 方式 | 优点 | 必须解决的问题 |
|---|---|---|
| 有界流式透传 | 不必把完整文件驻留内存，最接近 Native wire | model 字段改写困难；必须原样保留 boundary，并安全处理 headers |
| 有界解析后重建 | 可校验字段并改写 model | part/总大小、filename、重复字段、临时存储、取消和重建语义 |

不能把下游 `Content-Type` 简化成没有 boundary 的 `multipart/form-data`。若落盘，必须另有受限临时目录、唯一文件名、权限、容量、失败/取消清理；默认更适合使用有界 streaming multipart 或内存/流混合策略，且不持久化业务音频。

## capability 与预检

每个 operation 至少声明：

- model mapping 和允许的音频 formats/MIME；
- 单文件与总 multipart byte limit；
- response formats；
- transcription 的 language、timestamps、diarization、logprobs、stream 支持；
- translation 的目标语义与独立参数集合；
- JSON、文本/字幕或 SSE 的响应 media contract；
- 可重放预算、timeout 和取消行为。

filename 扩展名只能作为辅助信号，不能替代 content/media 校验。模型支持是 Upstream API 事实；OpenAPI 接受字段不代表所有 model 都接受。

## response 与 stream lifecycle

- 非流式结果按请求的 response format 和实际安全 `Content-Type` 透传，不经 Chat/Responses response parser。
- transcription SSE 使用独立 transcript event state machine，验证 UTF-8、framing、event size、顺序与 terminal。
- translation 若目标 profile 未声明 streaming，请求中的 stream 在 egress 前拒绝，不能静默忽略。
- 上游非成功 JSON/text error 必须先按 status 分类；不能因 body 恰好可解析为文本就作为转写成功。
- 下游 body 一旦开始，后续上游失败只能以该协议已有的错误/关闭语义结束，不能切换 candidate。

## retry、副作用与资源预算

音频文件理论上可重放，但大 body 会放大内存、带宽和费用。仅当 body 可安全 replay、请求仍在 attempt budget 内且下游尚未收到业务输出时才允许有限 retry。是否跨 Provider fallback 还要求 response format、language/translation 语义、timestamps/diarization 和模型质量契约一致。

超时发生时上游可能已经完整接收并开始处理。实现不得无限重放，也不得把 retry 当作幂等保证。取消需要同时终止 multipart 读取、上游发送、SSE 接收和 backoff。

## 安全与观测

- 不记录音频 bytes、transcript、prompt、filename 或 speaker labels；完整结果只返回调用者。
- 限制 part 数、字段长度、文件名、编码后/实际字节、请求时间与响应事件大小。
- 忽略下游提供的 Host、Authorization、upstream URL、proxy 与 hop-by-hop headers。
- trace 只记录 operation、Provider、Target、Public Model、status、attempt、bytes bucket 和终态等低敏事实。
- 合成 fixture 不包含真人语音、PII 或生产录音。

## TDD 与验收矩阵

| 层 | case |
|---|---|
| ingress | boundary 缺失/重复、重复 model/file、part/总大小、取消与清理 |
| transcription | WAV/MP3 合成 fixture、JSON format、stream events、timestamps/diarization capability gate |
| translation | 独立 path/model/format；不误接受 transcription-only stream/fields |
| adapter | model rewrite、multipart filename/media preservation、安全 header/auth |
| response | JSON/text/SSE 分流、错误 media type、fragmentation、terminal、EOF 与 byte limit |
| resilience | replay budget、首输出 commit、timeout、取消和不安全跨 Provider fallback 拒绝 |

真实 Provider 验收需要记录具体 model、音频格式、时长、response format、stream 模式与观察到的 wire；确定性 fixture 不证明识别准确率、说话人分离质量或生产延迟。

## 非目标

- 不实现 TTS、Realtime live transcription 或音频聊天；
- 不做音频转码、切片、降噪、VAD、说话人识别服务或字幕编辑；
- 不默认把上传文件持久化为 Files resource；
- 不将 transcription 与 translation response schema 合并；
- 不以 SDK 返回类型替代 HTTP multipart/media contract。
