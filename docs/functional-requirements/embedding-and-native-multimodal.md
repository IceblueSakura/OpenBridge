# Embeddings 与 Native 多模态扩展需求

## 状态与范围

**已批准的现阶段扩展目标。** 本文只增加两项产品目标：

1. OpenAI-compatible `POST /v1/embeddings`；
2. 现有 Chat Completions/Responses 的 Native JSON 多模态输入。

它不表示当前代码已经实现这些行为；实现与验证事实仍只写入[当前实现说明](../implementation-status/current-implementation.md)。两个目标不并行实施，每次仍由[当前开发焦点](../implementation-plans/current-focus.md)定义一个可观察行为。

协议字段、媒体形状和实现接缝分别见 [Embeddings 实现细节](../references/openai/implementation-details/01-embeddings.md) 与 [Chat/Responses 多模态实现细节](../references/openai/implementation-details/02-chat-responses-multimodal.md)。

## 1. Embeddings 用户结果

已认证客户端应能使用稳定 Embedding Public Model 调用 `POST /v1/embeddings`，而无需知道上游 Provider、真实 model、endpoint 或 credential。接口必须：

- 接受 OpenAI-compatible JSON 中的 string、string array、token array 和 token-array array 输入；
- 按所选固定接口能力校验 `encoding_format`、`dimensions`、批量与输入限制；
- 只把 Public Model 改写为 registry 中的真实 upstream model，保持合法 Native 字段；
- 返回有序 `data[]`、每项 `index`/`embedding`、响应 `model` 与 `usage`，不改变向量数值、编码、维度或顺序；
- 在无等价向量身份声明时禁止跨 Provider/模型 fallback；
- 对非法输入、不支持能力、响应形状错误和超限返回安全、稳定错误。

Embedding 是独立接口能力。它不得伪装成 Chat/Responses 文本生成，也不通过 Bridge、文本占位或网关本地向量变换实现。

## 2. Native JSON 多模态用户结果

已认证客户端应能在 Public Model 的 Chat 或 Responses 固定接口明确声明支持时，使用同协议 Native Route 转发以下输入：

| 输入 | 现阶段允许的来源 |
|---|---|
| image | 外部 URL、data URL/base64；仅按目标协议的标准 content part |
| file | inline file data 或外部 file URL；保留 filename/media 语义 |
| audio | Chat `input_audio` 的 base64 data + format；不据此推断 Responses audio input |

现阶段不得接受由 Provider/Target 签发的 `file_id`。在 OpenBridge 尚无 resource issuer/owner affinity 方案时，裸 ID 不能安全参与 retry/fallback；该请求必须在首次 egress 前稳定拒绝。

多模态能力必须逐协议、逐来源进入固定 Public Model interface contract。请求通过预检后仍保持完整 Route 顺序，不得为某个媒体 part 临时跳过较弱 Route、改选 Provider 或求能力并集。

Native 转发必须保持 content part 顺序、类型、URL/data、detail、filename、audio format、JSON/SSE 响应和原协议 terminal。除受信 model/path/auth/header 改写外，不得下载并替换媒体、把媒体转成文本、丢弃 part 或改变编码。

Chat ↔ Responses Bridge 对本阶段多模态请求保持 fail closed。只有未来建立逐字段、逐事件的无损表达证据后，才能另立需求开放某一具体转换方向。

## 3. 输入、URL 与数据保护

- JSON body、单个 content part、累计编码字节和 base64 解码后字节必须分别有界；不能只依赖现有总 JSON limit。
- URL 只能作为业务内容，不能控制 upstream base URL、Host、Authorization、credential、proxy 或 header transform。
- 外部 URL 需要明确 scheme、embedded credential、私网/loopback、redirect、DNS rebinding、下载时限和最大内容策略。若由上游而非 OpenBridge fetch，网关只能执行可证明的入站预检，并必须明确其无法控制的 Provider-side DNS、redirect 与下载边界。
- 原始文本、token array、URL query、filename、file data、audio/image bytes、embedding vector 和完整响应不得进入普通日志或 metrics label。
- 下游取消应停止当前发送/接收与待执行 backoff；首个业务输出后不得 fallback 或拼接另一个 Target 的结果。

## 4. Public Model 与能力

- Embedding model 使用独立任务模式和 `embeddings` interface，不以 `ModelMode::Chat` 或 generation capability 表达。
- Chat/Responses 分别公开 image/file/audio input 的已实现子能力；canonical model modality 只是模型事实，不能自动打开接口能力。
- `supported_parameters` 必须对应可执行预检规则。未知或尚未实现的字段不得因 Native passthrough 自动放行。
- 每个接口的公共能力由全部静态可执行 Route 保守相交；Embedding 的 dimensions、encoding/input forms 与生成协议的工具/stream 能力不混算。
- 标准与扩展 Models 接口继续隐藏 Provider、Target、Route、upstream model、credential 和运行状态。

## 5. 错误、retry 与证据

Embeddings 可在响应提交前按有限预算重放；跨 Target 只在 vector identity 等价得到显式 registry 证明时允许。多模态 JSON/SSE 沿用现有首输出 commit 与取消边界，但大 body 还必须服从单独 replay budget。

验收证据分层：

| ID | 应被保护的可观察行为 |
|---|---|
| EXT-01 | `/v1/embeddings` 的四种输入、model rewrite、float/base64、dimensions、data/index/model/usage 均满足固定 contract。 |
| EXT-02 | 无 vector identity 等价证明时不发生跨 Provider/模型 fallback，且向量不被网关转换。 |
| EXT-03 | Chat/Responses Native 保留混合 text/image/file/audio-input part 的顺序与 wire；未声明来源或格式在 egress 前失败。 |
| EXT-04 | `file_id` 在无 issuer affinity 时稳定拒绝；不会被尝试到多个 Target。 |
| EXT-05 | 多模态 Bridge 与 audio output 保持拒绝；不以丢字段、媒体转文本或 transcript 代替。 |
| EXT-06 | 编码/解码媒体 limit、URL policy、日志脱敏、首输出 commit 和取消均有确定性测试。 |
| EXT-07 | 独立 Python/OpenAI SDK 验证与真实 Provider 验证分别记录具体 endpoint、model、字段和证据边界；未运行层不声称兼容。 |

## 6. 现阶段非目标

- Images generation/edit/variation、Files lifecycle、Uploads、Vector Stores、File Search 和 Videos；
- `/audio/speech`、`/audio/transcriptions`、`/audio/translations`、Chat audio output 与 Realtime；
- Provider-issued `file_id`、resource ledger、跨 Provider resource migration 或媒体缓存；
- Chat ↔ Responses 多模态 Bridge、embedding Bridge、向量归一化/降维/索引/检索；
- 媒体下载代理、格式转换、OCR、转写、内容托管或通用安全扫描服务。
