# Public Model 与模型能力契约

## 状态

**当前目标。** 本文集中定义 Public Model 的公共身份、模型信息、固定接口能力、请求预检和 Models API
边界。它是“模型支持什么”的唯一功能需求入口；Route 执行、retry、fallback 与 cooldown 见
[路由与 Provider 韧性](provider-resilience.md)，当前实现事实见
[当前实现总览](../implementation-status/current-implementation.md)链接的功能专题。

## 1. 用户结果

客户端只需选择一个稳定 Public Model 和 Chat Completions、Responses 或 Embeddings 接口，即可在发起模型请求前读取同一份
静态能力契约。若所选模型不支持请求能力，OpenBridge 必须在任何上游调用前返回稳定错误；不得自动改选模型、 寻找能力更强的 Route
或静默删除请求字段。

模型信息用于能力展示和正确拒绝，不承担模型推荐、质量排序、成本优化或运行时调度。

## 2. 事实所有权与公开边界

| 层次                    | 拥有的事实                                                                          | 是否向下游公开                                     |
|-------------------------|-------------------------------------------------------------------------------------|----------------------------------------------------|
| Canonical Model         | 与 endpoint/credential 无关的模型名称、上下文、模态、参数和 reasoning 事实；已核实的 ChatGPT subscription profile 与一般 API 事实不同时，可使用独立 canonical profile identity | 模型事实经 Public Model 聚合；参数只经接口契约公开 |
| Provider / Upstream API | Provider 能力上界、served limits、协议、upstream model、state affinity 和 wire 映射 | 否                                                 |
| Route                   | 下游协议、Target、Upstream API、`Native`/`Bridged` 模式及配置顺序                   | 否                                                 |
| Public Model            | 稳定身份、生命周期、模型事实和每协议唯一固定能力契约                                | 是                                                 |
| RoutePlan / attempt     | 已接受请求的执行顺序、retry、fallback、credential 与 cooldown 状态                  | 否                                                 |

公共模型对象不得包含 Provider、Target、Route、upstream/canonical model id、endpoint、credential、header 或 wire
mapping，也不得包含健康、延迟、配额、价格、成本、指标、排行或 benchmark。运行指标通过独立的 startup-owned OTLP metrics
signal 导出，不属于 `PublicModelInfo`；上游 `/models` 与 probe 结果不能自动注册或扩大 Public Model。

Canonical profile identity 只用于区分不同的已核实模型事实，不代表 endpoint、credential 或请求方可选择的 Provider；其具体可调用性
仍必须由显式 Target、Upstream API、Route 和 Public Model 注册形成。

[Model 目录与 Provider 接入配置](model-catalog-configuration.md)目前是待定方案，不属于本契约或当前实施任务。
在它重新获得明确批准前，Canonical Model、Target/API、Route source 与 Public Model 继续由代码目录显式注册。

## 3. Public Model 身份、生命周期与可见性

- `id` 是客户端请求和资源路径使用的稳定单段标识，格式为
  `[A-Za-z0-9][A-Za-z0-9._:-]{0,127}`；包含 `/` 的上游模型名不得直接成为 Public Model id。
- `created` 是 Public Model 契约首次创建的稳定 Unix 秒，不使用进程启动时间。
- `name`、可选 `description` 和 `lifecycle` 是面向客户端的静态元数据。
- `active` 与 `deprecated` 模型仍可列出和调用；`retired` 模型对 list、retrieve 和模型请求统一表现为不可用。
- 没有任何静态可执行 Chat/Responses/Embeddings 接口的 Public Model 不进入可见目录。
- 标准列表、扩展列表、两个 retrieve 接口和请求预检必须读取同一个不可变 registry snapshot。

## 4. 模型事实与固定接口契约

### 4.1 公共对象

`PublicModelInfo` 必须包含：

- OpenAI 标准身份：`id`、`object`、`created`、`owned_by`；
- 生命周期和展示信息：`name`、`description`、`lifecycle`；
- 模型事实：任务、total/input/output context、输入/输出模态、tokenizer、知识截止和 reasoning；
- 接口契约：`chat_completions` 与 `responses` 各自至多一个生成接口能力对象，并可带协议内 source-aware `multimodal_input`；固定
  音频生成任务还可带 mode-aware `multimodal_output.audio`；
  `embeddings` 至多一个独立 Embedding 接口能力对象；
- schema 版本：首版固定为字符串 `"1"`。Embeddings interface 首次加入前该扩展契约尚未发布，因此直接修正 v1 DTO、序列化、OpenAPI
  和测试，不增加无意义的 v2、legacy 字段镜像或双写兼容层。

模型事实是模型本体的安全公共上界；模型请求是否可调用某能力，必须以目标 `interfaces` 项为准。某协议没有 可执行 Route 时，其接口值为
`null`。canonical Model 的参数事实只参与编译各接口的
`supported_parameters`，模型事实层不得再公开一份不能直接用于请求放行的重复列表。

### 4.2 未知语义

- 布尔能力使用 `supported`、`unsupported`、`unknown`；只有 `supported` 能通过请求预检。
- 未知 token 限制、tokenizer、知识截止或日期使用 JSON `null`。
- 数组只包含已确认值，必须去重并确定性排序；空数组表示没有可公开保证的值。
- `unknown` 不能按“上游也许支持”提升为 `supported`；`unsupported` 不能伪装成 `unknown`。

OpenRouter canonical model 的 `context_length` 是模型目录公开的上下文上限，而不是独立的
`max_input_tokens` 字段。OpenBridge 将这项已确认的模型级上限投影到 `max_context_tokens` 和
`max_input_tokens`；`top_provider.max_completion_tokens` 只用于 `max_output_tokens`。不把总上下文减去 最大输出做未经
OpenRouter 声明的残差推导；若某个具体 Upstream API 更窄，应通过
`UpstreamApiModelRules` 明确收窄。

### 4.3 固定契约计算

每个 Public Model 对 Chat Completions、Responses 和 Embeddings 分别只有一个固定契约。registry 在启动时把该 operation 全部
静态启用、可执行的 Route 作为契约输入，并按以下规则保守相交：

| 字段                                                                                | 计算规则                                                                                                              |
|-------------------------------------------------------------------------------------|-----------------------------------------------------------------------------------------------------------------------|
| 布尔能力                                                                            | 全部 Route 明确支持才是 `supported`；任一明确不支持则为 `unsupported`；证据不足保持 `unknown`                         |
| token 上限                                                                          | 全部 Route 都有已知值时取最小值；任一未知则为 `null`                                                                  |
| 模态、参数、reasoning level                                                         | 取集合交集并稳定排序                                                                                                  |
| image/file/audio source、inline encoding、format、detail allowed、可验证 media type | 按目标协议分别取集合交集；detail default 必须一致，任一必需集合为空则对应媒体输入子契约不可公开                       |
| audio output mode、format、voice、encoding/container、采样参数与上限                 | 按 JSON/SSE mode 分别保守相交；条件 format 不得压平，任一 mode 无完整 framing/累计预算时不得公开                      |
| media part、URL 长度、inline 编码/解码字节上限                                      | 取全部 Route 保证值与 gateway hard limit 中的最小值；累计字节只统计 inline payload                                    |
| reasoning 输出形态                                                                  | 全部 Route 形态相同时公开该值，否则为 `unknown`                                                                       |
| `Bridged` Route                                                                     | 只贡献当前转换器完整支持的公共子集；本阶段对 image/file/audio source 与 audio output 贡献空集                         |

Embedding 接口不使用生成协议的 token-output、tool、reasoning 或 stream 字段。它应独立保守相交 input forms、默认/可显式请求的
output encoding、默认维度、可请求 dimension domain 和输入/批量限制；encoding 与 dimensions 都不得压缩成布尔值。公开
`max_inputs` 还必须被 gateway batch/response budget 收窄，不能接受一个必然产生本地超限成功体的请求。不同 vector identity
未被显式证明等价时，不得编译进同一可 fallback 契约。Embedding Route 只允许 Native，不从 Chat/Responses Bridge 派生。

Chat/Responses 的 `modalities.input`/`modalities.output` 只是摘要。具体 image/file/audio 请求还必须匹配 `multimodal_input` 中的协议
part、source、inline encoding、format/detail/media type 与 limits；音频生成还必须匹配 `multimodal_output.audio` 的 JSON/SSE mode、
format、voice、framing 与累计预算。嵌套 content part 字段不加入顶层 `supported_parameters`；task-specific `asr_options`/`audio` 只在
对应 interface 顶层公开。共同编译规则见[扩展导航](embedding-and-native-multimodal.md)，闭合集合分别由
[图片](native-image.md)、[文件](native-file.md)和[音频](native-audio.md)功能页拥有。

音频输入还必须绑定业务用途：`content_understanding`、`speech_recognition` 与 `voice_conditioning` 不能因为都使用 Base64/URL 或
`input_audio` 而聚合。模型任务、输入用途、输出语义和 typed parameter 任一不同，都必须使用独立 interface/Public Model contract。

能力不得按字段求并集，也不返回 `guaranteed + profiles`、conditional capability 或按 Route 展开的公共视图。
`previous_response_id` 除了要求全部 Responses Route 明确支持，还要求这些 Route 唯一解析到同一个 Upstream
Target/API；存在多个潜在签发者时必须公开为 `unsupported`，并从接口 `supported_parameters` 删除。

若同一 canonical Model 由多个 Provider Target 提供，只有代码目录将对应 route source 显式列入同一 Public Model 时才形成聚合；模型
ID 相同不能自动新增候选。聚合后每个协议的全部静态可执行 Route 仍共同参与上述 保守交集，不能只按首选 Provider 计算公共契约。

## 5. 请求预检与禁止能力路由

模型请求必须遵循固定顺序：

1. 分析请求 operation、Public Model，以及该接口的 input form、encoding/dimensions、streaming、tool、媒体
   part/source/format/detail、URL 长度、inline 编码/解码字节、结构化输出、reasoning、state 和输出限制等事实。
2. 查询所选 Public Model 的目标接口固定契约。
3. 对所有已建模请求能力执行一次 fail-closed 预检。
4. 不支持或未知时立即返回错误，不创建 RoutePlan，不调用 Provider adapter 或 transport。
5. 预检通过后，严格按 Public Model 的配置顺序构造完整 RoutePlan。

代码目录从多个 Provider source 生成配置顺序时，对每个下游协议先按 source 声明顺序排列全部 Native Route， 再按相同顺序排列
Bridge Route；生成后这一 Vec 即为固定配置顺序，运行时不得再按 Provider 或模式重排。

以下行为一律禁止：

- 根据请求能力选择另一个 Public Model；
- 因某条 Route 能力较弱而跳过它；
- 因后续 Route 能力较强而提升公共契约；
- 根据能力、模型字符串、价格、健康或 benchmark 重排 Route；
- 把一条 Route 的 tool、image、reasoning 或 token 优势与另一条 Route 的能力做字段并集。

Route 候选资格只取决于协议匹配和静态启停；Target/API 绑定、顺序及 `Native`/`Bridged` 模式均来自固定配置。 reasoning wire
映射只能在选定候选的 Provider egress 请求准备阶段改写 wire 副本，不得写入 RoutePlan，也不能 改变候选资格或顺序。若完整
`BridgePlan` 无法表示已通过 公共预检的请求，整个请求必须失败，不能跳过该 Bridge 去选择其他 Route。 运行期
cooldown、429/5xx、timeout、credential rotation 和首输出前 fallback 属于可用性执行，不是能力路由；
`previous_response_id` 等 state affinity 可以禁止跨 Target fallback，但不能选择能力更强的候选。

## 6. Models API 契约

| 接口                                | 成功响应                                        |
|-------------------------------------|-------------------------------------------------|
| `GET /v1/models`                    | `object: "list"` 与严格四字段 `StandardModel[]` |
| `GET /v1/models/{model}`            | 一个严格四字段 `StandardModel`                  |
| `GET /openbridge/v1/models`         | `object: "list"` 与完整 `PublicModelInfo[]`     |
| `GET /openbridge/v1/models/{model}` | 一个完整 `PublicModelInfo`                      |

共同要求：

- 四个接口使用与生成接口相同的静态 Bearer 认证。
- `StandardModel` 严格只有 `id`、`object: "model"`、`created` 和 `owned_by: "openbridge"`。
- 同一 snapshot 下，retrieve 必须与对应列表元素逐字段相同；列表按 Public Model id 确定性排序。
- 未知、retired 或当前不可用模型返回 HTTP 404、`model_not_found`，`param` 为 `model`，不得区分内部存在性。
- 固定接口契约不支持请求时返回 HTTP 400、`unsupported_model_capability`，并保证上游调用次数为零。
- 已识别但当前 OpenBridge 尚未实现的协议能力可以返回独立稳定的 `unimplemented_request`，不得尝试透传猜测。
- 第一版不提供分页、搜索、排序参数、模型 ACL、能力过滤或动态刷新。

## 7. 启动时校验

registry 必须在监听前拒绝：

- 非法 Public Model id、零值 `created`、空白展示字段或不一致生命周期时间；
- total/input/output context 为零，或输入/输出上限超过 total context；
- 显式模态集合为空或重复；
- 空 Route 列表、重复 Route 或未知引用；
- Upstream API 规则扩大 canonical Model，或收窄后产生不一致事实；Embedding identity、dimension、encoding 或 input-form 声明矛盾；
- Chat/Responses 媒体 source/format/detail/media type 集合为空、重复、协议错配，或 limits 为零/相互矛盾；
- reasoning 状态、level、参数声明或 wire mapping 不一致；
- Upstream API 能力超过 Provider contract 上界。

## 8. 功能验收要求

| ID       | 应被保护的用户可观察行为                                                                                                                                               |
|----------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| MODEL-01 | 标准 list/retrieve 只返回四字段对象，且详情与列表元素相同。                                                                                                            |
| MODEL-02 | 扩展 list/retrieve 返回同一个固定能力对象；参数只由目标接口公开，且不包含部署、凭据、价格或运行状态。                                                                  |
| MODEL-03 | active/deprecated 模型可见；retired 或无可执行接口的模型不可见、不可调用。                                                                                             |
| MODEL-04 | 较弱首选 Route 与较强后续 Route 的交集仍拒绝能力请求，且不发生 egress。                                                                                                |
| MODEL-05 | 能力预检通过后保留全部配置 Route 的原顺序，不按请求能力跳过或重排。                                                                                                    |
| MODEL-06 | unknown 能力 fail closed；token 上限与集合按保守交集计算。                                                                                                             |
| MODEL-07 | Chat、Responses 与 Embeddings 能力相互隔离，不能用一个接口的能力扩大另一个接口。                                                                                       |
| MODEL-08 | 未知模型和 retired 模型统一返回安全 `model_not_found`；能力不足返回 `unsupported_model_capability`。                                                                   |
| MODEL-09 | registry 在启动时拒绝非法身份、生命周期、上下文、模态、引用和能力扩大。                                                                                                |
| MODEL-10 | Embeddings dimension domain、Chat/Responses source-aware 输入与 mode-aware 音频输出由 Models projection 和 preflight 共享，不能由 bool、Native passthrough 或请求期 Route 过滤扩大。 |

确定性 Rust/HTTP 测试只证明本地 registry、序列化、预检和 Route 顺序；不证明真实 Provider 当前能力、外部 SDK、负载、长期运行或
LiteLLM/OpenRouter 目录新鲜度。

## 9. 非目标

- 根据能力、质量、成本或 benchmark 自动选模；
- 按请求能力筛选、打分、加权或重排 Route；
- 在 Models API 中暴露 deployment、endpoint、credential、健康、价格、配额、指标或 benchmark；运行指标只通过独立 OTLP metrics
  signal 导出，不属于模型目录或模型能力契约；
- 从 LiteLLM、OpenRouter、Provider `/models` 或 probe 动态发现和注册模型；
- 模型推荐、自动迁移、alias resolution、ACL、分页搜索或 capability query API；
- 在未实现协议语义前，仅因模型本体声称支持就放行 hosted/custom tool、audio/file、state、embedding 参数或 opaque reasoning。

## 关联文档

- [产品范围](product-scope.md)
- [网关 API 与客户端兼容](gateway-api-compatibility.md)
- [扩展能力导航及共同规则](embedding-and-native-multimodal.md)
- [Embeddings 能力](embeddings.md)
- [Native 图片能力](native-image.md)
- [Native 文件能力](native-file.md)
- [Native 音频能力](native-audio.md)
- [待定 Model 目录与 Provider 接入配置](model-catalog-configuration.md)
- [配置、凭证与受信边界](configuration-and-credentials.md)
- [路由与 Provider 韧性](provider-resilience.md)
- [LiteLLM/OpenRouter 模型信息综合调研](../references/cross-project/model-information-comparison.md)
- [当前实现总览](../implementation-status/current-implementation.md)
