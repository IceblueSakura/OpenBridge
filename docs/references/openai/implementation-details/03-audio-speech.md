# Text-to-speech 协议实现细节

**目标状态：** 仅作协议参考，不在现阶段 1/2 实施范围。

## 范围与 wire contract

目标协议是 `POST /v1/audio/speech`。当前官方 endpoint 接受 `application/json`，根据请求模式返回原始音频字节或 `text/event-stream` 音频事件。它不是 Chat audio output，也不是 Realtime session。

核心请求事实包括 model、text input、voice、输出音频格式，以及部分 model/profile 支持的 instructions、speed 和 stream format。字段与枚举必须从所选 Provider/model contract 得出；不能把某个 OpenAI model 的 voice/format 列表提升为通用能力。

官方资料：[Create speech](https://developers.openai.com/api/reference/resources/audio/subresources/speech/methods/create)、[Audio and speech](https://developers.openai.com/api/docs/guides/audio) 与 [Text to speech](https://developers.openai.com/api/docs/guides/text-to-speech)。

## operation 与 capability

Speech 应有独立 operation，不通过 `ApiProtocol::ChatCompletions` 或 `GenerationCapabilities` 表示。最小接口能力包括：

- 允许的 model mapping；
- built-in voice 与 custom voice ID 的类别；
- output audio formats；
- raw audio 与 SSE stream mode；
- input character/token/byte limits；
- instructions、speed 等 model-specific 参数；
- 最大响应字节、首字节与完整请求 timeout；
- voice resource 的 issuer affinity。

Public Model 可以代表 TTS model，但扩展 Models DTO 应把 `audio_speech` 与 Chat/Responses interface 分开。voice 不是 model，custom voice/consent ID 不能出现在公共能力枚举或日志中。

## 请求与响应分流

处理链必须在出站前确定响应模式：

1. 认证、JSON media type、body limit 和 Public Model 预检；
2. 校验 input、voice、format、stream mode 及 model-specific 参数；
3. 从 registry 选择受信 endpoint、真实 model、credential 和 voice policy；
4. 对 raw mode 只做有界二进制流转发，对 SSE mode 使用 Speech 专用事件分类；
5. 只返回安全的 `Content-Type`、可验证的长度/disposition 和 allowlist rate-limit/request-id header；
6. 在 EOF、协议 terminal、上游错误或下游取消处记录唯一终态。

raw audio 的正常终止是 body EOF。不能要求 `[DONE]`、`response.completed` 或 Chat finish reason。SSE mode 才能进入 SSE framing，但其事件词汇也不能复用 Responses terminal discriminator。若响应 media type 与请求的 mode/format 不一致，应 fail closed，而不是把错误 JSON 当音频返回。

## retry 与流生命周期

- 在上游尚未接受或下游尚未收到业务字节时，可按明确错误分类进行有限 retry；大请求仍受 replay budget 约束。
- 第一个音频业务字节或 SSE event 提交后，不得切换 credential/Target 拼接另一段音频。
- timeout 后是否已经产生计费/副作用通常不可知；跨 Target fallback 必须作为目标能力显式批准，而非沿用文本生成默认值。
- 下游取消要停止上游读取和待执行 backoff；不得把取消记录为成功 EOF。
- raw stream 不需要在内存中聚合完整音频；但必须执行总字节上限和传输速率/超时约束。

## voice 与安全边界

内置 voice name 只有在多个 candidate 明确共享同一语义时才可跨 candidate 使用。custom voice、consent 或 clone 资源必须绑定签发 Provider/Target/credential scope；未知 issuer 直接拒绝。

不得记录 input 文本、voice resource ID、音频 bytes 或 base64 event。若提供语音生成服务，还需要在产品层明确用户披露、声音授权和保留政策；协议转发成功不证明这些合规要求已经满足。

## TDD 与验收矩阵

| case | 必须证明 |
|---|---|
| raw mp3/wav/pcm 等已声明格式 | Content-Type、字节顺序、EOF、取消和 byte limit 正确 |
| SSE audio | fragmentation、事件大小、唯一 terminal 和 EOF-before-terminal 处理正确 |
| response mismatch | JSON error、错误 media type 或非法 event 不作为成功音频返回 |
| voice gate | 未声明 voice/custom ID 在 egress 前失败，target-bound ID 不跨 Target |
| model rewrite | 下游 Public Model 不泄露真实 upstream model，其他合法字段 Native 保留 |
| retry | 首输出前有限 attempt；首字节后不 fallback；取消停止重试 |

canonical fixture 只使用合成短音频和虚构 voice ID。真实音质、延迟、voice 可用性、格式支持和 Provider 限额必须另做外部验收。

## 非目标

- 不把 TTS 伪装成 Chat/Responses 文本生成；
- 不做语音克隆、voice consent 管理、格式转码或音频后处理；
- 不实现 STT、Realtime 或双向通话；
- 不缓存、拼接或持久化生成音频；
- 不从 raw audio 推断 SSE 事件，也不从 transcript 重建 audio。
