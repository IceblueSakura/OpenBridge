# Embeddings 与 Native 多模态扩展需求

## 状态与范围

**两项目标均已批准；Embeddings 已完成当前实现，Native 多模态尚未进入开发焦点。** 现阶段扩展范围仍只包含：

1. OpenAI-compatible `POST /v1/embeddings`；
2. 现有 Chat Completions/Responses 的 Native JSON 多模态输入。

当前没有活动开发焦点；Native 多模态仍是已批准但未获得实施授权的独立目标。扩展 Models
接口公开的类型化输入能力必须能被同一份不可变执行接口直接预检；只有该固定契约明确允许的输入形状、来源和选项才能进入 Native
egress。它不是对所有 OpenAI 媒体 API 的并行实现授权；Embeddings
的完成事实与实际证据只记录在[当前实现总览](../implementation-status/current-implementation.md)链接的功能专题中。

扩展 schema 在本次实现前尚未发布，因此采用首版最佳实践迁移：保持 `schema_version: "1"` 并直接修正
DTO、parser、registry、OpenAPI、配置与测试，不提供旧字段镜像、兼容 alias、双读写、默认回退或弃用窗口。已完成的 Embeddings
实现没有顺带改动多模态 reserved bool；多模态进入后续独立焦点时再原子替换其 bool/保留位设计。

外部协议字段与媒体形状分别见 [Embeddings 协议调研](../references/openai/protocol-details/01-embeddings.md)
与 [Chat/Responses 多模态协议调研](../references/openai/protocol-details/02-chat-responses-multimodal.md)。

## 1. 能力事实的分层

扩展能力必须保持以下四层分离：

| 层                               | 拥有的事实                                                                       | 不得替代的事实                                         |
|----------------------------------|----------------------------------------------------------------------------------|--------------------------------------------------------|
| Canonical Model                  | task、输入模态、原生向量维度等模型本体事实                                       | endpoint wire、媒体来源、参数、限制或 Route            |
| Provider/Upstream API            | 某个受信 endpoint 实际支持的输入形状、来源、格式、detail、维度域和 served limits | 下游 Public Model 身份或动态请求选择                   |
| Public Model execution interface | 全部静态可执行 Route 的保守交集，以及与该交集绑定的固定候选顺序                  | Provider/Target 拓扑、credential、运行时健康或能力并集 |
| Request requirements             | 本次请求实际使用的 input form、source、format、detail、数量和可直接计算的字节数  | 重新筛选、跳过或重排 Route 的依据                      |

Canonical modality 只能证明模型可能消费某类输入，不能自动打开 API 能力。`image_input: true`、`file_input: true`、
`audio_input: true` 或笼统的 `multimodal: true` 都不足以成为可执行公共契约。

## 2. Embeddings 用户结果

已认证客户端应能使用稳定 Embedding Public Model 调用 `POST /v1/embeddings`，而无需知道上游 Provider、真实 model、endpoint 或
credential。接口必须：

- 接受 OpenAI-compatible JSON 中的 string、string array、token array 和 token-array array 输入；
- 拒绝空字符串、空集合、混合类型数组、非法 token 值和 profile 未声明的输入形状；
- 按所选固定接口能力校验 `encoding_format`、`dimensions`、批量、可直接计算的 token-array 数量和字节限制；
- 只把 Public Model 改写为 registry 中的真实 upstream model，保持该 Native profile 明确允许的其他字段；
- 将成功响应的 `model` 归一为下游 Public Model，并保持有序 `data[]`、每项 `object`/`index`/`embedding`、响应 `object` 与
  `usage`；
- 不改变向量数值、base64 内容、维度、项目顺序或 index 语义；
- 在没有等价向量身份声明时禁止跨 Provider/模型 fallback；
- 对非法输入、不支持能力、响应形状错误和超限返回安全、稳定错误。

Embedding 是独立接口能力，不得伪装成 Chat/Responses 文本生成，也不通过 Bridge、文本占位或网关本地向量变换实现。

### 2.1 `interfaces.embeddings` 最小公共契约

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
contract，不能用字符或 UTF-8 字节估算冒充本地预检。客户端仍可读取已知 token 上界，但证据必须区分 local rejection 与
upstream enforcement。

内部 vector identity 至少约束 immutable model/checkpoint、tokenizer/input encoding、默认与可选维度、归一化/距离语义及编码语义。它只用于
registry 校验与 fallback 安全，不得把 upstream model 或 Provider identity 暴露给下游。

## 3. Native JSON 多模态用户结果

已认证客户端应能在 Public Model 的 Chat 或 Responses 固定接口明确声明支持时，使用同协议 Native Route
转发下列输入。第一版允许集合必须逐协议建模：

| 协议 part               | 可建模来源                             | 现阶段边界                                                                                    |
|-------------------------|----------------------------------------|-----------------------------------------------------------------------------------------------|
| Chat `image_url`        | `remote_url`、`data_url`               | 省略/显式 `detail` 分别服从该 Upstream API 声明的 default 与 allowed domain                   |
| Responses `input_image` | `remote_url`、`data_url`、`file_id`    | 第一版只开放 URL/data URL；`file_id` 拒绝                                                     |
| Chat `file`             | `inline_data`、`file_id`               | 第一版只开放 profile 声明的 raw-base64 `inline_data`；不虚构 `file_url`、MIME 字段或 `detail` |
| Responses `input_file`  | `inline_data`、`remote_url`、`file_id` | 第一版开放 inline/URL；`detail` 只对已声明的文件 profile 生效；`file_id` 拒绝                 |
| Chat `input_audio`      | `inline_base64`                        | format 必须属于接口明确集合；OpenAI 当前标准形状使用 `wav`/`mp3`                              |
| Responses audio input   | 无                                     | 不从 Chat 字段或模型 audio modality 推断支持                                                  |

每个 part 必须满足其协议的一选一 source 规则。例如 Responses `input_file` 不能同时携带 `file_data`、`file_url` 和
`file_id`；Chat `file` 不能把 URL 或 MIME 填入不存在的标准字段。inline file 必须保留并校验 filename、profile 声明的
`raw_base64`/`data_url` encoding，以及 wire 实际携带的 media type 语义。

Chat image/file/input_audio 只在官方 user-message content union 的位置有效；出现在 developer/system/tool/assistant
等不允许角色时必须按 malformed request 拒绝。Responses 只检查其标准 input item/message content 位置，不能递归搜索任意同名字段。

现阶段不得接受由 Provider/Target 签发的 `file_id`。在 OpenBridge 尚无 resource issuer/owner affinity 方案时，裸 ID 不能安全参与
retry/fallback；该请求必须在首次 egress 前稳定拒绝。

### 3.1 类型化多模态能力

Chat/Responses interface 分别提供可选的 `multimodal_input.image`、`.file` 和 `.audio` 子契约：

- image：来源集合、inline media type 集合、detail default/allowed domain、part 数、URL 长度及 inline 编码/解码字节上限；
- file：来源集合、inline encoding 集合、可验证的 media type/filename 规则、detail default/allowed domain 与适用文件类别、part
  数、URL 长度及 inline 编码/解码字节上限；
- audio：来源集合、format 集合、part 数和 inline 编码/解码字节上限；
- interface 共同限制：媒体 part 总数、累计 inline 编码字节和累计 inline 解码字节。

子契约缺失或集合为空即表示不支持。嵌套 content part 名称、source、format 和 detail 不伪装成顶层 `supported_parameters`；
`supported_parameters` 继续只描述该 endpoint 的顶层可选字段。

## 4. 能力编译与请求预检

公共能力必须由与同一个 `ModelExecutionInterface` 绑定的全部静态可执行 Route 保守编译：

- input form、显式 encoding、source、format、media type 和显式 detail 等集合取交集；encoding/detail default 必须一致；
- part、字节、token 和 dimension 上限取能够保证的最小值；
- Embeddings `max_inputs` 还必须用 checked arithmetic 被 gateway batch/JSON response budget、最大公开维度、允许 encoding
  的最坏序列化上界和固定 envelope 收窄；无法证明至少一个输入的合法响应受限时 registry 启动失败；
- explicit encoding 与 dimension 离散集合/区间求可表示的交集；交集为空但 default 一致时把 `allowed` 设为 `null` 并移除对应
  `supported_parameters`，default 不一致时拒绝聚合；
- 默认 embedding encoding/维度、向量 identity 或协议 wire 不一致时，不得把候选聚合成一个可 fallback 的 Embeddings
  interface；
- Bridged Route 对本阶段 image/file/audio 来源贡献空集；因此只要它仍属于该接口候选，新多模态能力就不能被公开为整个接口保证；
- 未知或缺乏证据的能力按不支持处理，不能因 Native passthrough、首选 Route 较强或某个模型目录声明 modality 而提升。

请求分析必须冻结实际使用的 input form、source、inline encoding、format、detail、part 数、URL 长度、inline
编码字节和可安全解码后的字节数。preflight 只将这些事实与固定接口比较；通过后 planning 仍使用完整候选顺序，不能根据某个媒体
part 临时跳过较弱 Route、改选 Provider 或求能力并集。

## 5. Native 保真与 Bridge 边界

Native 转发必须保持 content part 顺序、类型、URL/data、detail、filename、audio format、JSON/SSE 响应和原协议 terminal。除受信
model/path/auth/header 改写及下游 Public Model response projection 外，不得下载并替换媒体、把媒体转成文本、丢弃 part 或改变编码。

Chat ↔ Responses Bridge 对本阶段多模态请求保持 fail closed。只有未来建立逐字段、逐事件的无损表达证据后，才能另立需求开放某一具体转换方向。

## 6. 输入、URL 与数据保护

- JSON body、单个 content part、累计 inline 编码字节和 base64 解码后字节必须分别有界；remote URL 另有长度上限，不能只依赖现有总
  JSON limit。
- URL 只能作为业务内容，不能控制 upstream base URL、Host、Authorization、credential、proxy 或 header transform。
- 第一版远程来源只接受有长度上限的绝对 HTTPS URL，拒绝 userinfo、localhost 及显式 loopback/link-local/private/reserved IP
  literal；OpenBridge 不主动下载或解析重定向。
- 当媒体由 Provider fetch 时，OpenBridge 无法证明 Provider-side DNS、redirect、下载时限和内容上限；这部分必须作为真实
  Provider 验收边界明确记录，不能把入站语法检查描述成完整 SSRF 防护。
- data URL/file data 必须使用允许的 media type 与规范 base64，并在分配大缓冲前完成有界长度检查；URL query、filename、file
  ID、原始媒体与解码错误上下文不得进入普通日志。
- 原始 embedding 文本、token array、向量、base64、完整响应与 `user` 值不得进入日志或 metrics label。
- 请求与 Provider attempt 使用低基数 operation 维度；Embeddings 固定标识为 `embeddings_create`。其 `prompt_tokens`/
  `total_tokens` 只进入 input/total counter，不虚构 output token、generation throughput 或流式 terminal。
- 下游取消应停止当前发送/接收与待执行 backoff；首个业务输出后不得 fallback 或拼接另一个 Target 的结果。

## 7. Retry、fallback 与证据

Embeddings 只有在请求 body 不超过独立 replay budget 且响应尚未提交时才可有限重放；超过 replay budget 但仍在 request hard
limit 内的合法请求只执行第一次 attempt，不因内部重放优化被额外拒绝。跨 Target 只在 vector identity 等价得到显式 registry
证明时允许。当前实现限制为单条 Native Embeddings Route，不实现向量等价聚合。多模态 JSON/SSE 沿用现有首输出 commit 与取消边界，但大
body 同样只能执行一次。

验收证据分层：

| ID     | 应被保护的可观察行为                                                                                                                                     |
|--------|----------------------------------------------------------------------------------------------------------------------------------------------------------|
| EXT-01 | `interfaces.embeddings` 的 forms、encoding default/domain、dimension default/domain、limits 与参数列表来自同一执行接口，且与 `/v1/embeddings` 预检一致。 |
| EXT-02 | `/v1/embeddings` 四种输入、model 双向投影、float/base64、dimensions、data/index/object/usage 均满足固定 contract，向量不被转换。                         |
| EXT-03 | 无 vector identity 等价证明时不发生跨 Provider/模型 fallback；当前单 Route 切片的 retry、取消和响应提交边界可确定复现。                                  |
| EXT-04 | Chat/Responses 公开并执行逐协议 image/file/audio source、inline encoding、format、detail 和 limit 交集；未声明形状在 egress 前失败。                     |
| EXT-05 | Native 上游收到的 mixed text/media part 顺序与 wire 保持；请求不会按媒体能力筛选、跳过或重排候选。                                                       |
| EXT-06 | `file_id`、多模态 Bridge、Responses audio input 与 Chat audio output 稳定拒绝，不以丢字段、媒体转文本或 transcript 代替。                                |
| EXT-07 | remote URL 长度、inline 编码/解码媒体 limit、HTTPS 语法 policy、日志脱敏、首输出 commit 和取消均有确定性测试。                                           |
| EXT-08 | 标准 Models 仍为四字段；扩展 Models 不暴露 Provider、Target、Route、upstream model、credential、vector identity 或运行状态。                             |
| EXT-09 | 独立 Python/OpenAI SDK 与真实 Provider 验证分别记录实际 endpoint、model、字段和证据边界；未运行层不声称兼容。                                            |

## 8. 现阶段非目标

- Images generation/edit/variation、Files lifecycle、Uploads、Vector Stores、File Search 和 Videos；
- `/audio/speech`、`/audio/transcriptions`、`/audio/translations`、Chat audio output 与 Realtime；
- Provider-issued `file_id`、resource ledger、跨 Provider resource migration 或媒体缓存；
- Chat ↔ Responses 多模态 Bridge、embedding Bridge、向量归一化/降维/索引/检索；
- 媒体下载代理、格式转换、OCR、转写、内容托管或通用安全扫描服务；
- 通过请求期 capability routing、动态 Provider discovery 或未知字段 passthrough 扩大固定公共契约。
