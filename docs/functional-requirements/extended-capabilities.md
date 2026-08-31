# 扩展能力合同

本文集中定义 Embeddings、Native 图片/文件/音频和 Images Generations 的固定接口、资源边界与验收约束。

## 共同能力边界

本节定义所有扩展 operation 共享的能力分层、编译和预检规则；实现状态见[当前实现](../implementation-status/current-state.md)。

### 1. 能力事实分层

| 层 | 拥有的事实 | 不得替代的事实 |
|---|---|---|
| Canonical Model | task、输入/输出模态、上下文或向量本体事实 | endpoint wire、媒体来源、参数、限制或 Route |
| Provider/Upstream API | 受信 endpoint 的输入/输出形状、来源、格式、选项与 served limits | 下游 Public Model 身份或动态请求选择 |
| Public Model execution interface | 全部静态可执行 Route 的保守交集及其固定候选顺序 | Provider/Target 拓扑、credential、健康或能力并集 |
| Request requirements | 本次请求实际 form、role、source、format、数量及可直接计算的资源事实 | 重新筛选、跳过或重排 Route 的依据 |

Canonical modality 只能证明模型可能消费或产生某类数据，不能自动打开 API 能力。粗粒度
`image_input`/`file_input`/`audio_input`/`audio_output` bool 或 `multimodal: true` 都不能替代完整的可执行
profile。

### 2. 能力编译与请求预检

- 公共能力由同一 `ModelExecutionInterface` 的全部静态可执行 Route 保守编译：集合取交集，数值上限取
  可保证的最小值，default 必须一致。
- 未知或没有完整 contract 的能力按不支持处理，不能因 Native passthrough、首选 Route 较强、Models list
  或 canonical modality 提升。
- Bridged Route 只贡献 converter 能完整表达的共同子集；image/file/audio source 与 audio output 不贡献
  Bridge 能力。
- 请求分析冻结对应功能页规定的 form、role、source、encoding、format、detail、voice、数量与资源 facts；
  preflight 只与固定 interface 比较。
- preflight 通过后仍使用完整固定候选顺序，不能根据媒体 part、encoding 或输出 mode 跳过较弱 Route、
  改选 Provider 或求能力并集。
- 标准 Models 保持四字段；扩展 Models 不暴露 Provider、Target、Route、upstream model、endpoint、
  credential、内部 identity 或运行状态。

### 3. Native 保真与 Bridge

Native 转发必须保持请求 part/item 顺序、类型、source data、格式/选项、成功响应和原协议 terminal。除受信
model/path/auth/header 改写及 Public Model response projection 外，不得下载并替换媒体、转换 embedding、把媒体
转成文本、丢弃字段或改变编码。

Native保真不能绕过canonical request/response decode或SSE lifecycle validation；只有同协议、capability check通过且无需跨origin重解释时，才可在验证后保留request语义与Provider私有字段，并原样保留response与SSE bytes。未知可移植语义、非法identity/lifecycle、越界body/event和terminal前EOF继续fail closed。

Chat-to-Responses 与 Responses-to-Chat Bridge 对媒体请求保持 fail closed；只有对应功能需求定义了逐字段、逐事件
的完整转换契约后，才可开放某个具体方向。

### 4. 资源与数据保护

- JSON body、单个 content part、累计 inline encoded bytes 与安全解码后的 bytes 必须分别有界；remote URL
  另有长度上限。
- URL 只是业务内容，不能控制 upstream base URL、Host、Authorization、credential、proxy 或 header transform。
- remote source 只接受有界 absolute HTTPS URL，拒绝 userinfo、localhost 及显式 loopback/link-local/private/
  reserved IP literal；OpenBridge 不主动下载，也不解析 redirect。
- Provider-side DNS、redirect、下载时限、远端 MIME/大小与内容安全不由入站语法检查替代。
- inline data 必须在大分配前检查 encoding、media type 与 byte limit。
- URL query、filename、resource ID、原始媒体、Base64、transcript、向量、完整响应和敏感错误上下文不得进入
  普通日志、trace attribute 或 metric label。
- 不同功能的 token、seconds、media bytes 与 vector 统计不得混写。

### 5. Retry 与取消

- 只有 request body 未超过 replay budget 且 response 尚未提交时，才能按对应功能的幂等边界有限重放；
  超过 replay budget 的合法请求只执行第一次 attempt。
- 下游取消必须停止当前发送/接收与待执行 backoff；首个业务输出提交后不得 retry、fallback 或拼接另一响应。
- 跨 Target fallback 需要功能特有的等价 identity；同名模型、相同 modality 或共用 endpoint 都不是证明。

### 6. 共同非目标

- 请求期 capability routing、动态 Provider discovery 或未知字段 passthrough；
- 未经功能页明确授权的 Images edit/variation、Files/Uploads/Vector Stores/Videos/Realtime 资源或会话 API；
- 媒体下载代理、格式转换、OCR、通用转写、内容托管、向量检索或通用安全扫描；
- Provider-issued resource identity 的跨账户、跨 Target 或跨 Provider 猜测与迁移。

外部协议事实见[OpenAI 细粒度协议调研](../references/openai/README.md)。

## Embeddings

### 范围

本页只定义 OpenAI-compatible `POST /v1/embeddings` 的输入、输出、能力、资源和失败边界。它不定义图片、文件、音频或其他
Chat/Responses 媒体能力；共同的能力分层与固定 Route 规则见
[扩展共同规则](extended-capabilities.md)。实现与验证事实见[实施现状](../implementation-status/README.md)。

### 1. 用户结果

已认证客户端应能使用稳定 Embedding Public Model 调用 `POST /v1/embeddings`，而无需知道上游 Provider、真实 model、endpoint 或
credential。接口必须：

- 接受 OpenAI-compatible JSON 中的 string、string array、token array 和 token-array array 输入；
- 拒绝空字符串、空集合、混合类型数组、非法 token 值和 profile 未声明的输入形状；
- 按所选固定接口能力校验 `encoding_format`、`dimensions`、批量、可直接计算的 token-array 数量和字节限制；
- 只把 Public Model 改写为 registry 中的真实 upstream model，保持该 Native profile 明确允许的其他字段；
- 将成功响应的 `model` 归一为下游 Public Model，并保持有序 `data[]`、每项 `object`/`index`/`embedding`、响应 `object` 与
  `usage`；
- 不改变向量数值、维度或 index 语义；只有具体 Target/Upstream API 的 fixed interface 明确声明时，Provider adapter 才可在
  JSON finite number 数组与 IEEE-754 little-endian float32 标准 Base64 wire 之间做有界、确定性的表示转换，并在提交前按完整 index
  集合规范化顺序；转换保持维度和 float32 数值语义，但不承诺保留超出 float32 的 JSON 数值精度；
- 在没有等价 vector identity 声明时禁止跨 Provider/模型 fallback；
- 对非法输入、不支持能力、响应形状错误和超限返回安全、稳定错误。

Embeddings 是独立 operation，不得伪装成 Chat/Responses 文本生成，也不通过 Bridge、文本占位或网关本地数值向量变换实现。固定
Provider adapter 的 float32/Base64 wire 表示转换不改变 vector identity，不属于归一化、降维或 embedding Bridge。

### 2. `interfaces.embeddings` 公共契约

一个可调用的 Embeddings interface 至少公开：

| 字段                   | 语义                                                                                                                                |
|------------------------|-------------------------------------------------------------------------------------------------------------------------------------|
| `input_forms`          | `string`、`string_array`、`token_array`、`token_array_array` 的非空保证集合                                                         |
| `encoding.default`     | 省略 `encoding_format` 时保证的 `float` 或 `base64` wire                                                                            |
| `encoding.allowed`     | `null` 或可显式请求的 `float`/`base64` 非空集合；不得由网关本地转换补足                                                             |
| `dimensions.default`   | 省略 `dimensions` 时保证返回的正整数维度                                                                                            |
| `dimensions.allowed`   | `null`、闭区间或离散集合；`null` 表示请求不得携带 `dimensions`                                                                      |
| `limits`               | 有效批量项数、单输入/总 token 上界，以及 `locally_counted_input_forms`；部署级 request、JSON response 与 replay budget 另行统一执行 |
| `supported_parameters` | 除必填 `model`/`input` 外可执行的顶层可选字段，例如 `encoding_format`、`dimensions`、`user`                                         |

第一版 `locally_counted_input_forms` 只包含 token-array 两种形状；string/string-array 的 token 上界是 Provider-enforced
contract，不能用字符或 UTF-8 字节估算冒充本地预检。

内部 vector identity 至少约束 immutable model/checkpoint、tokenizer/input encoding、默认与可选维度、归一化/距离语义及编码语义。
它只用于 registry 校验与 fallback 安全，不得向下游暴露 upstream model 或 Provider identity。

### 3. 编译、预检与响应预算

- input form、显式 encoding 和 dimension domain 取全部静态可执行 Route 的交集；default 必须一致。
- explicit encoding/dimension 交集为空但 default 一致时，把 `allowed` 设为 `null` 并移除相应 `supported_parameters`；default
  不一致时拒绝 registry 编译。
- `max_inputs` 必须用 checked arithmetic 被 gateway batch/JSON response budget、最大公开维度、允许 encoding 的最坏序列化上界和
  固定 envelope 收窄；无法证明至少一个输入的合法响应受限时启动失败。
- 请求分析冻结实际 input form、encoding、dimension、批量和可直接计算的 token/byte facts；通过 preflight 后不得按请求重新筛选
  Route。
- 成功体必须在下游提交前完成有界 JSON shape 校验；网关只投影 Public Model，不转换向量或编码。

### 4. 重放、取消与数据保护

- 请求 body 不超过 replay budget 且响应尚未提交时才可有限重放；超过 replay budget 但仍合法的请求只执行第一次 attempt。
- 只有 vector identity 等价得到显式 registry contract 时才允许跨 Target；不得以同名模型推断等价。
- 下游取消必须停止发送、接收和待执行 backoff；任何成功 body byte 提交后不得 retry 或拼接第二个响应。
- 原始文本、token array、向量、Base64 与 `user` 不得进入日志、trace attribute 或 metrics label。
- Embeddings operation 固定使用低基数 `embeddings_create`；只记录明确返回的 input/total token，不虚构 output token 或生成速度。

### 5. 验收

| ID     | 应被保护的可观察行为                                                                                                                         |
|--------|----------------------------------------------------------------------------------------------------------------------------------------------|
| EMB-01 | interface 的 forms、encoding、dimension、limits 与参数列表来自同一执行接口，并与 `/v1/embeddings` preflight 一致。                           |
| EMB-02 | 四种输入、model 双向投影、float/base64、dimensions、data/index/object/usage 满足固定 contract；允许的 wire re-encoding 按 little-endian float32 语义确定转换，完整乱序 index 被规范化，缺失/重复/越界继续拒绝。 |
| EMB-03 | 无 vector identity 等价证明时不发生跨 Provider/模型 fallback；retry、取消、响应预算和首输出 commit 可确定复现。                             |
| EMB-04 | 标准 Models 仍为四字段；扩展 Models 不暴露 Provider、Target、Route、upstream model、credential、vector identity 或运行状态。                |

### 6. 非目标与参考

非目标包括 embedding Bridge、向量归一化、降维、缓存、索引、检索和根据向量能力动态选路。

- [OpenAI Embeddings Create 调研](../references/openai/embeddings-create.md)

## Native 图片输入

### 范围

本页只定义 Chat `image_url` 与 Responses `input_image` 的同协议 Native 输入能力。它不定义 Images generation/edit/variation、
文件、音频、视频或跨协议媒体转换；共同规则见[扩展共同规则](extended-capabilities.md)。实现与验证事实见
[实施现状](../implementation-status/README.md)。

### 1. 用户结果与 wire

| 协议 part               | 可建模来源                          | 固定边界                                                                  |
|-------------------------|-------------------------------------|---------------------------------------------------------------------------|
| Chat `image_url`        | `remote_url`、`data_url`            | 只在 user message content 中有效；省略/显式 `detail` 分别服从 profile     |
| Responses `input_image` | `remote_url`、`data_url`、`file_id` | 固定契约只开放 URL/data URL；没有 resource affinity 时必须拒绝 `file_id` |

图片 part 必须出现在协议规定的 user content union 中；developer/system/tool/assistant 或任意递归同名字段都不能被当作合法图片输入。
Native 转发保持 mixed text/image part 的顺序、类型、URL/data、detail 与原协议 JSON/SSE terminal，只允许受信 model/path/auth/header
改写及 Public Model response projection。

### 2. `multimodal_input.image` 公共契约

每个 Chat/Responses interface 的图片子契约必须明确：

- 允许的 `remote_url`/`data_url` source；
- data URL media type 集合；
- `detail` default 与 allowed domain；
- 单请求图片 part 数、单 URL UTF-8 长度；
- 单项和累计 inline encoded/decoded byte 上限。

静态 executable profile 必须是一个完整 envelope，而不是可独立组合的 source slice 和多个 limit：

- envelope 拥有正数 `max_parts`；source 使用 `RemoteUrl(remote_limits) | DataUrl(inline_profile) |
  RemoteUrlAndDataUrl { remote, data }` 判别联合；
- Remote payload 只拥有 URL byte limit，data payload 只拥有非空、唯一的 media type set 与完整 inline encoded/decoded 单项及累计预算；
- `detail` 使用 `OmittedOnly { default } | Explicit { default, allowed }` 语义。两者都允许省略 wire 字段；只有 `Explicit`
  接受显式值且 `allowed` 必须非空、唯一。省略后的已知 default 与显式 allowed domain 是独立事实，default 不要求属于 allowed。

Public Model 按全部可执行 Route 逐 source 相交：Remote URL limit 取最小；Data URL media type 取交集并对其四项预算取保守最小。
Data media type 交集为空只移除 Data URL；若 Remote payload 仍完整则降为 Remote-only，所有 source 都消失才关闭整个图片子契约。
`max_parts`、单项预算和累计预算分别取最小后，累计 encoded/decoded 还必须以 checked wide arithmetic 收紧到
`per-item × max_parts` 的可达上限，并通过同一 checked envelope 重新验证。`detail` default 必须完全一致；任一候选为
`OmittedOnly` 时交集也是 `OmittedOnly`，全部为 `Explicit` 时 allowed 取交集，空交集安全降为保留共同 default 的
`OmittedOnly`。

扩展 Models 保持既有 flat JSON shape，但它只是上述 union 的只读投影：Remote-only 的 media type 与四项 inline limit 投影为空/`0`，
Data-only 的 URL limit 投影为 `0`，Both 投影两组正数。`0` 不是 core/registry 配置状态或 source 证据；请求 preflight 必须读取同一
编译结果中的 private owned source contract，不得反向读取 DTO。嵌套 part 字段不加入顶层 `supported_parameters`；
`modalities.input` 只作为摘要，不能替代 typed profile。

### 3. URL、Base64 与请求预检

- remote source 只接受有长度上限的 absolute HTTPS URL，拒绝 userinfo、localhost 和显式 loopback/link-local/private/reserved
  IP literal；OpenBridge 不主动下载图片或解析 redirect。
- data URL 必须使用 profile 允许的 media type 与规范 Base64，并在分配大缓冲前检查编码/解码上限。
- checked profile 的最小 wire-reachable limit 是 9 个 UTF-8 byte（`https://a`）、4 个 encoded byte（一个 Base64 quantum）和 1 个
  decoded byte。累计预算必须至少覆盖一个单项且不得超过 `per-item × max_parts`；这些只是类型可达性下界，不是对 Provider
  operational limit 的推测。
- 请求分析冻结 role、part/source、media type、detail、数量、URL 长度和 inline byte facts；非法或超限输入在首次 egress 前失败。
- Responses `file_id` 继续作为 analyzer 可识别的 wire fact，但不进入静态 source-payload union；没有 resource identity、ownership、
  affinity 与 limits 的完整 profile 时必须在首次 egress 前 fail closed。
- Provider fetch 的 DNS、redirect、下载时限、远端 MIME/大小和内容安全属于真实 Provider 边界，入站 URL 检查不能冒充完整 SSRF
  防护。

### 4. Route、Bridge 与数据保护

- 公共能力是同一 interface 全部静态可执行 Route 的保守交集；图片请求通过 preflight 后仍保留完整固定候选顺序。
- Bridged Route 对图片 source 贡献空集；图片请求不得通过 Chat ↔ Responses Bridge，也不得按请求跳过较弱 Route。
- 网关不得下载、转码、OCR、重排、缓存或把图片替换成文本。
- URL query、原始图片、Base64、完整响应和解码错误上下文不得进入普通日志或 metrics label。
- 大 body 超过 replay budget 时只执行第一次 attempt；首个下游业务输出后不得 retry/fallback。

### 5. 验收

| ID     | 应被保护的可观察行为                                                                                                               |
|--------|------------------------------------------------------------------------------------------------------------------------------------|
| IMG-01 | Chat/Responses 分别从 source-payload union 公开 typed source、media type、detail 与 limit，并与请求 preflight 使用同一 fixed owned interface。 |
| IMG-02 | Native 上游收到原有 mixed text/image part 顺序和 wire；请求不按图片能力跳过、筛选或重排候选。                                     |
| IMG-03 | 非 user 位置、`file_id`、非法 URL/Base64/media type/detail、不可达 profile 与超限输入在 egress 前稳定拒绝。                      |
| IMG-04 | URL/data source 的限制、日志保护与 Native 保真使用同一固定 profile；未声明的格式、尺寸或 source 在 egress 前拒绝。             |

### 6. 非目标与参考

非目标包括 Images API、图片生成/编辑/variation、file-backed resource、媒体下载代理、OCR、格式转换和多模态 Bridge。

- [OpenAI Chat 图片输入调研](../references/openai/images-chat-input.md)
- [OpenAI Responses 图片输入调研](../references/openai/images-responses-input.md)
- [Xiaomi MiMo 图片协议与真实观察](../references/providers/xiaomi-image.md)

## Native 文件输入

### 范围

本页只定义 Chat `file` 与 Responses `input_file` content part 的同协议 Native 输入能力。它不定义 Files/Uploads/Vector Stores/File
Search 生命周期；共同规则见[扩展共同规则](extended-capabilities.md)。实现与验证事实见
[实施现状](../implementation-status/README.md)。

### 1. 用户结果与 source 规则

| 协议 part              | 可建模来源                             | 首个目标边界                                                                                    |
|------------------------|----------------------------------------|-------------------------------------------------------------------------------------------------|
| Chat `file`            | `inline_data`、`file_id`               | 只开放 profile 声明的 inline data；不虚构 `file_url`、MIME 字段或 `detail`                      |
| Responses `input_file` | `inline_data`、`remote_url`、`file_id` | 开放 profile 声明的 inline/URL；`detail` 只对明确文件类别生效；无 resource affinity 时拒绝 ID |

每个 part 必须满足协议的一选一 source 规则。Responses `input_file` 不能同时携带 `file_data`、`file_url` 与 `file_id`；Chat
`file` 不能接受标准 wire 中不存在的 URL/MIME/detail 字段。inline file 必须保留并校验 filename、profile 声明的
`raw_base64`/`data_url` encoding，以及 wire 实际携带的 media type 语义。

### 2. `multimodal_input.file` 公共契约

文件子契约至少明确：

- 允许的 inline/remote source 与 inline encoding；
- 可验证 media type、filename 和适用文件类别；
- `detail` default/allowed domain 及其适用类别；
- part 数、URL 长度和单项/累计 inline encoded/decoded byte 上限。

任一必需集合为空、default 不一致或某个 Route 无法保证该 source 时，对应接口不得公开文件能力。嵌套字段不加入顶层
`supported_parameters`。

### 3. Resource identity、预检与保真

- OpenBridge 尚无 Provider/Target resource issuer、owner 与 continuation affinity 时，必须在首次 egress 前拒绝 `file_id`。
- remote URL 服从有界 absolute HTTPS 语法策略；OpenBridge 不下载文件，也不能证明 Provider-side DNS、redirect、MIME 或大小。
- inline data 必须在大分配前完成 encoding、media type、filename 和字节上限检查；请求分析冻结实际 source/encoding/detail/limit
  facts。
- Native 转发保持 filename、inline data、URL、detail、part 顺序与原协议 terminal；不得提取文本、转换格式、缓存或签发新 ID。
- Bridged Route 对 file source 贡献空集；不能根据本次 file source 跳过 Route 或执行多模态 Bridge。

### 4. 重放与数据保护

- 超过 replay budget 但仍在 request hard limit 内的合法文件请求只执行第一次 attempt。
- 首个业务输出后不得 retry/fallback；下游取消必须停止发送/接收和 backoff。
- URL query、filename、file ID、原始文件、Base64、完整响应与解析错误上下文不得进入普通日志或 metrics label。

### 5. 验收

| ID      | 应被保护的可观察行为                                                                                                            |
|---------|---------------------------------------------------------------------------------------------------------------------------------|
| FILE-01 | Chat/Responses 各自只接受标准 content part、source one-of、encoding、filename/detail 与 typed limit。                          |
| FILE-02 | 无 issuer/owner affinity 时 `file_id` 在 egress 前稳定拒绝；不会跨 Provider/Target 猜测或迁移资源。                            |
| FILE-03 | Native wire、part 顺序和 metadata 保持；请求不进入 Bridge、下载、文本提取、转换或请求期能力路由。                              |
| FILE-04 | URL/inline limit、日志脱敏、replay budget、取消和首输出 commit 共享同一固定失败边界。                 |

### 6. 非目标与参考

非目标包括 Files lifecycle、Uploads、Vector Stores、File Search、资源 ledger、跨 Provider migration、媒体托管与通用安全扫描。

- [OpenAI Chat 文件输入调研](../references/openai/files-chat-input.md)
- [OpenAI Responses 文件输入调研](../references/openai/files-responses-input.md)

## Native 音频

### 范围

本页定义五种不可互换的 Chat Native 音频任务：通用音频理解、ASR/STT、TTS、以文本约束音色的 VoiceDesign，
以及以参考音频约束音色的 VoiceClone。本页不定义 OpenAI `/audio/speech`、`/audio/transcriptions`、
`/audio/translations`、Responses audio 或 Realtime；共同规则见[扩展共同规则](extended-capabilities.md)，实现与验证事实见
[实施现状](../implementation-status/README.md)。

### 1. 任务身份与不可替代性

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

### 2. `mimo-v2.5` 通用音频理解

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

### 3. MiMo ASR/TTS 最小目标契约

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

### 4. 音色设计与音色克隆边界

- 音色设计的自然语言描述是生成条件，不得作为普通 TTS 可选 voice 名称或通用模型 instruction 处理。
- 音色克隆的参考音频是 `voice_conditioning` resource，不得进入 `content_understanding` 或 `speech_recognition` profile；固定 contract 只
  暴露独立 source/format/byte limit，授权确认、保留期和日志脱敏策略仍是后续媒体治理边界。
- 两个变体各自使用独立 canonical task、Chat Native profile 与失败边界；普通 TTS 成功不提升其他模型能力。
- VoiceDesign 只接受自然语言 voice description；VoiceClone 只接受独立 `audio.voice` reference resource，不建立跨模型
  voice identity 或资源复用。gateway 只做 shape/source/format/size 预检，不承诺授权、保留期或媒体内容验证。

### 5. `multimodal_output.audio` 与响应预算

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

### 6. 启动门禁、预检、保真与 Bridge

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

### 7. Retry、取消与数据保护

- 通用音频理解只在 body 未超过 replay budget、响应尚未提交且仍是同一 target/model 时有限 retry；不得 fallback 到 ASR。
- ASR 只有在 body 未超过 replay budget、响应尚未提交且仍是同一 target/model 时才能有限 retry。
- TTS 首个目标不自动 retry，因为再次合成可能重复计费并产生不同音频。
- 所有音频任务禁止跨 task/model fallback；任何 JSON body、text delta 或 audio delta 提交后不得 retry、rotation 重放或拼接响应。
- 原始音频、Base64、transcript、TTS 目标/风格文本、voice sample 和 Provider request ID 不得进入普通日志、metrics label、probe
  report 或 fixture。
- `audio_tokens`、seconds、audio bytes 与文本 token 必须保持语义，不把 PCM bytes 当 token、transcript 长度当时长或 chunk 数当速度；
  gateway 只保留并透传上游 JSON/SSE，不自行计算或重解释这些字段。

### 8. 验收

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

### 9. 非目标与参考

非目标包括 `/audio/*`、Responses audio、Realtime、未进入固定 profile 的 remote/multi-audio/格式、ASR 方言承诺、未单独验证的
VoiceDesign/VoiceClone 扩展格式与 voice identity/resource 复用。

- [OpenAI Chat 音频输入与输出调研](../references/openai/audio-chat-input-output.md)
- [Xiaomi MiMo 全模型语音能力与调用途径](../references/providers/xiaomi-audio.md)

## Images Generations

本文只定义下游 `POST /v1/images/generations` 的固定行为、失败语义、安全边界与非目标；实现与验证事实统一见
[实施现状](../implementation-status/README.md)。

### 1. 下游契约

- endpoint 为 `POST /v1/images/generations`，JSON-only，同一静态下游 Bearer 认证。
- strict request catalog 识别当前 OpenAI Images Create 字段：`model`、`prompt`、`n`、`size`、`response_format`、
  `output_format`、`stream`、`partial_images`、`background`、`moderation`、`output_compression`、`quality`、`style`、`user`；
  未知顶层字段在 egress 前以 400 `invalid_request_error` 拒绝。
- 已知标准字段先完成类型/枚举/range 分析，再由 model-bound preflight 判断支持；当前 qwen profile 对不支持的标准字段返回
  400 `unsupported_model_capability` 和准确 `param`，不得降级为 unknown field 或静默丢弃。
- OpenAI optional `null` 视为省略。qwen 支持 `n`、`size: "auto" | "宽x高"`、`response_format: "url"`、
  `output_format: "png"`、`stream:false` 和 `user`；`b64_json`、jpeg/webp、`stream:true`、partial image、quality/style/background/
  moderation/compression 均 fail closed。
- qwen profile 额外识别 DashScope 顶层扩展（兼容 OpenAI SDK `extra_body`）：`prompt_extend`、`prompt_extend_mode`、
  `enable_thinking`、`negative_prompt`、`seed`、`watermark`。扩展必须由 `interfaces.images.dashscope_extensions` 明确公开；
  无该 profile 的模型按字段拒绝。
- 成功响应固定为 `{created, data: [{url}], output_format: "png", size: "宽x高"}`；`data` 数量等于解析后的 `n`，
  size 来自已验证 DashScope usage，URL 是 Provider 短期签名 URL，不构成永久 resource identity。

### 2. 能力契约与预检

- Images 是独立 operation（`images_generations`），不进入 Chat/Responses Bridge，无生成协议语义。
- Provider ceiling 由 `ImagesGenerationsCapabilities` 拥有：`n` 上限、size 域（每边与面积）、
  `response_format` 域、标准参数集合及可选 `DashScopeImagesCapabilities`；Public Model 的 `interfaces.images` 是全部固定候选的
  保守交集，default 必须一致，DashScope extension profile 仅在所有候选完全一致时公开。
- preflight 一次解析标准字段、DashScope extension facts 与响应预期并冻结；超域、未声明字段或冲突依赖在首次 egress 前返回
  400，不能改选模型或 Route。`prompt_extend:false` 与显式 mode/thinking child 冲突，按字段返回 `invalid_request_error`。
- DashScope 默认明确冻结为 `prompt_extend:true`、`prompt_extend_mode:"direct"`、`enable_thinking:true`、
  `watermark:false`；`seed` 为 `[0, 2147483647]`，`negative_prompt` 必须非空白 string。
- ImageGeneration canonical task 固定为 text→image 生成；它不继承 Generation task 的 reasoning、streaming
  或 function-tool 语义。

### 3. 上游 wire 与响应验证

- 每个 Provider adapter 只使用其受信注册的 Native 路径；业务请求不能覆盖 URL、模型、credential 或认证 header。
- OpenAI 请求向 DashScope 原生请求的转换只做已证明映射：`prompt`→`input.messages`、`n`→`parameters.n`、
  `size` 的 `x`→`*`，`size:"auto"` 转为省略；`user`、`response_format`、`output_format`、`stream:false` 不离开网关。
- 已通过 extension preflight 的六个 DashScope 字段才进入 `parameters`；省略字段使用冻结默认，禁止 adapter 接受任意 JSON passthrough。
- 上游响应在 commit 前整体验证：按 body `code` 字段识别业务错误、逐 choice 提取非空图片 URL、`usage.output_image_count`
  与解析后的 `n` 一致、width/height 为正整数、投影后 JSON 不超过 response budget。任何违反 fail closed，不提交部分结果。
- success body 读取按 `too_large | body_transport | invalid_contract | success` 闭合分类；超限、读取失败、提前 EOF、损坏 JSON、
  output/usage mismatch 全部在 commit 前返回 502。读取中取消释放 body source；validated success 后的 downstream drop 不回写 Provider failure。
- 验证后的图片数量、宽、高写入独立 Images histogram；不得伪装成 token usage。

### 4. Retry、取消与数据保护

- Images generation 不自动 retry/fallback：请求可能已被接受、计费或产生结果，网络不确定时不得盲目重放。
  多个固定 candidate 在启动期聚合为保守公共交集，request-time 只选择配置优先级第一项；candidate 顺序不改变 Models 能力，
  Target 的 omission defaults 必须与 Provider 相等，size domain 必须存在满足 side/area/aspect 的整数 `WxH`；disjoint 或不可达
  domain 不公开显式 `size`。每次请求仍是单 candidate、单 credential、单 attempt；即使 credential pool
  或 candidate set 有其他成员，401/429/timeout/transport failure 也不 rotation、fallback 或重放。
- connect/TLS/response-headers timeout 固定返回 504 `upstream_timeout`；其他 transport failure 返回安全
  502 `upstream_error`。每次实际 send 精确记录一个 Provider attempt，HTTP、transport、timeout、success headers 与取消各自唯一终结。
- 下游取消终止上游请求；响应提交后不得重放或拼接。
- prompt、上游 body、错误上下文与图片 URL 不进入普通 tracing、OTLP trace attribute 或 metric label。显式开启的有界下游
  response-body 内容日志仍按全局开发日志策略观察最终客户端响应；它不是普通 telemetry 或原始 Provider wire dump。

### 5. 非目标

- `b64_json` 响应、`stream:true`/partial-image SSE、异步任务轮询（`X-DashScope-Async`）与任务查询；
- Images edit/variation、I2I 编辑、多 Provider/多 Target fallback 与请求期 capability routing；
- 网关下载、缓存、代理或延长 Provider 图片 URL；OCR、内容安全或质量承诺。

### 6. 验收项

- IMG-GEN-01：标准字段目录与未知字段区分；已知但不支持字段按准确 `param` zero-egress；
- IMG-GEN-02：OpenAI `null`、`size:"auto"`、PNG 与 `stream:false` omission-equivalent 路径通过；其余标准域按 model profile 拒绝；
- IMG-GEN-03：DashScope 六字段类型、range、依赖和缺失 extension profile 均 fail closed；冻结默认与显式值准确进入 native wire；
- IMG-GEN-04：成功响应按 bounded body、choice/usage 双重校验，投影 URL、PNG 和实际 size；仅 validated success 的图片
  count/width/height 进入非 token metrics，所有 body/contract failure 为零 usage；
- IMG-GEN-05：单 attempt、无重放；prompt、negative prompt 与 URL 不进入遥测。
