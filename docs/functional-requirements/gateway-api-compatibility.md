# 网关 API 与客户端兼容需求

## 状态

**当前目标。** 本文定义 OpenBridge 对下游客户端可见的 API、认证、原生 HTTP/SSE 语义和兼容边界；不规定内部模块、converter
形态或实现顺序。当前已经由代码和测试证明的范围以[当前实现总览](../implementation-status/current-implementation.md)链接的功能专题为准。

## 1. 用户结果

受信用户应能把本地 Agent 或 OpenAI-compatible SDK 指向一个稳定的 OpenAI-compatible base URL，使用私有用户表中分配的
Bearer API Key 与 Public Model 调用服务。主要调用路径不得要求客户端知道上游 Provider、真实模型、URL、凭证或候选切换细节。

初期的兼容目标按优先级为：

1. OpenAI SDK 的 Chat Completions 与 Responses HTTP JSON/SSE；
2. OpenAI-compatible Embeddings、Chat/Responses 同协议 Native 多模态输入，以及按任务分离的 Chat Native 音频理解、ASR/TTS；
3. 独立 Python 脚本或 curl 的最小 HTTP/header/SSE 复现；
4. 只有在明确声明时，才验证 Codex、Hermes 等具体客户端的 profile、transport 与 tool-loop 行为。

“某个请求能被转发”不等于“某个 Agent 已完整兼容”。每项声明必须限定 endpoint、stream、tool、continuation、Provider 与实际验证版本。

## 2. 接口与认证

| 接口                                                             | 功能要求                                                                                                              | 不包含的语义                                                                                 |
|------------------------------------------------------------------|-----------------------------------------------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------|
| `GET /healthz`                                                   | 提供不访问上游凭证的最小本地存活信息；不得泄露 route、Upstream Target 或 secret。                                     | Provider 健康探测、控制面或客户端管理。                                                      |
| `GET /v1/models`、`GET /v1/models/{model}`                       | 按[模型能力契约](model-information-and-capability-contract.md)返回严格的 OpenAI 标准四字段 list/retrieve。            | 扩展能力、上游模型或部署信息。                                                               |
| `GET /openbridge/v1/models`、`GET /openbridge/v1/models/{model}` | 返回同一 Public Model 目录的模型事实和 Chat/Responses/Embeddings 固定能力契约。                                       | Provider/target/route、credential、健康、价格或动态发现。                                    |
| `POST /v1/chat/completions`                                      | 支持已声明能力范围内的 Chat JSON/SSE，并按[图片](native-image.md)、[文件](native-file.md)和[音频](native-audio.md)需求提供 Native 媒体能力。 | 多模态 Bridge、未声明模型的 audio output、专用媒体/资源 API 或 hosted tool 的默认兼容承诺。 |
| `POST /v1/responses`                                             | 支持已声明能力范围内的 Responses JSON/SSE，并按[图片](native-image.md)和[文件](native-file.md)需求提供 Native 媒体输入。 | 多模态 Bridge、Responses audio/WebSocket、资源 retrieve/cancel/store/background/conversation API。 |
| `POST /v1/embeddings`                                            | 支持独立 Embedding Public Model 的 OpenAI-compatible JSON 请求/响应。                                                 | streaming、向量转换/存储/检索，或无等价证明的跨模型 fallback。                               |
| `POST /mcp`                                                      | 提供 MCP `2026-07-28` Streamable HTTP 本地入口，支持 server discovery、静态 `hello` 目录和无副作用调用。              | 动态工具、Provider Bridge、外部 side effect、旧版 session/initialize、资源、prompt 或常驻通知 stream。 |

业务 endpoint 必须使用用户表分配的静态 Bearer API Key。用户表只在启动时读取，不提供在线 key issuance、scope、即时撤销、配额或
billing identity；变更需要重启。认证失败与未知/不支持 endpoint 必须在进入路由或上游调用前结束，且不泄露配置细节。

### 2.1 MCP 本地工具入口

- MCP endpoint 与 Chat/Responses 中的 function-tool wire 转发相互独立。它只提供显式注册的本地工具，不把 Public Model、Provider、
  Target、Route 或上游 credential 暴露为 MCP tool。
- endpoint 只接受带现有下游 Bearer token 的 `POST /mcp`，并使用 MCP 正式协议版本 `2026-07-28`。`server/discover` 必须声明
  `tools` capability；`tools/list` 必须按确定性顺序返回唯一的 `hello`，其 closed `inputSchema` 只接受一个必需字符串 `name`。
- `tools/call` 只执行 `hello`：有效调用返回一个文本 content block `Hi, {name}!`，不读取配置、registry、文件、网络或 Provider。
  无效 `hello` argument 返回 `isError: true` 的工具结果；未知工具返回 JSON-RPC `-32602` protocol error。
- 每个请求必须携带 `application/json` body、同时接受 JSON/SSE 的 `Accept`、`MCP-Protocol-Version` 与 `Mcp-Method` header，
  并与 JSON-RPC body 中的 method、protocol version 和 per-request client capabilities 一致；`tools/call` 还必须携带与 body tool
  name 一致的 `Mcp-Name`。缺失、畸形或不一致的 metadata 必须在任何工具执行前以稳定 JSON-RPC error 失败。
- 当前 endpoint 不接受任何 `Origin` header。带 Origin 的请求必须以 `403` 失败；无 Origin 的本地客户端仍受 loopback listener、
  Bearer 认证、全局请求体上限、请求 ID、敏感 header 与终态观测保护。
- 当前不提供旧版 `initialize`/`initialized` handshake、`Mcp-Session-Id`、独立 GET SSE stream 或 DELETE session lifecycle；
  已认证的 `GET /mcp` 与 `DELETE /mcp` 返回 `405`，缺少凭证仍先返回 `401`。未实现的 RPC method 返回 HTTP `404` 与
  JSON-RPC `-32601`。

## 3. 请求、Public Model 与安全边界

### 3.1 Public Model 与 routes

- 下游只能提供已配置的 Public Model；它表示 OpenBridge 对下游提供的稳定服务契约，而不是某个上游模型名的透明别名。身份、生命周期、固定能力计算、Models
  API 和错误语义统一由[Public Model 与模型能力契约](model-information-and-capability-contract.md)定义。
- 请求能力只在所选 Public Model 边界预检一次，不参与选模、Route 候选资格、顺序或 fallback。预检通过后，Route 仍按配置顺序固定
  Upstream Target、Upstream API、下游 operation 和执行模式；generation `Native` 要求协议相同，`Bridged` 要求协议相反且通过完整
  `BridgePlan` preflight，Embeddings 只允许同 operation Native。
- 每个 generation Public Model 必须显式声明一种类型化 Route ordering strategy。`NativeFirst` 对每个 downstream protocol 先按
  source 顺序排列全部 Native，再排列 Bridge；`SourceFirst` 对每个 downstream protocol 先保持 source 顺序，再在同一 source 内将
  Native 排在 Bridge 前。自动 Bridge 只补全整个 Public Model 缺失的 Native protocol coverage；显式 Bridge surface 可以在其他
  source 已有 Native coverage 时保留。两种策略都在启动期冻结，运行时不得因请求能力、价格、健康或 Provider 名称重新打分或重排。
- 服务对上游只使用选中 route 的真实模型名、协议、endpoint 与 credential；下游不能通过 body、query 或 header 指定上游
  URL、模型、credential、provider family、route、转换脚本或 header 转换规则。Provider 的受信代码 hook 可以按编译期规则增添、替换、转换或删除普通
  header，但认证、cookie、Host 与 proxy header 始终隔离。
- 请求开始后，Public Model、RoutePlan、credential pool binding 与注册表版本保持固定；无状态 attempt 可按策略选择 pool
  member。

### 3.2 输入保护

- 仅接受端点契约允许的 content type、JSON body 和受配置约束的大小；无法安全解析的请求在 egress 前返回稳定错误。
- 请求分类必须先识别 operation，再按 operation 解析 `stream`、input form、function/custom/hosted
  tool、并行工具、结构化输出、multimodal、reasoning、`previous_response_id`、background/store 与相应限制等会影响固定契约或状态边界的特征。
- Chat/Responses 下游请求的顶层字段必须先按源协议的代码内类型化目录分类；未知字段即使值为 `null` 也必须在 egress 前以稳定
  `unknown_parameter` 拒绝，不能因“目标 Provider 也许支持”而进入 Native 或 Bridge。已知字段的 `null`/`false` 是否表示未请求能力，
  只由该字段的类型化语义决定，不形成通用绕过规则。
- 服务为每个请求生成或传播安全的 request id，用于响应和受控诊断；该 id 不是 client identity、tool identity 或聚合指标
  label。

## 4. Native Path 与流式语义

当下游与上游协议一致且请求已通过 Public Model 固定契约预检与输入归一化时，Native Path 是兼容性基线：它只做受信路由、模型、认证、显式
reasoning level wire 映射和已验证的普通生成提示忽略，保留其他已知且被接口接受的请求 JSON，并保持上游响应中的未知合法 JSON
字段/SSE event，不经过通用 IR 重渲染。level
映射必须属于选定 Upstream API 的代码注册规则，映射源必须已由 canonical Model 声明， 目标必须是安全 wire
值；不得由业务请求提供映射或用映射扩大 Public Model 支持的下游 level 集合。 canonical reasoning level vocabulary 为
`none`、`minimal`、`low`、`medium`、`high`、`xhigh`、`max`； 每个 Model 仍须显式声明实际支持的子集。`none` 是调用方显式要求禁用
reasoning，不等同于缺少 reasoning 字段。

每个 generation Public Model 必须静态选择 reasoning input policy。`strict` 只接受固定接口 `levels` 中的值；
`clamp_positive_floor` 仅处理正向序列 `minimal < low < medium < high < xhigh < max`：选择不高于请求值的最高可执行档位，若请求值低于
全部可执行正向档位则选择最低可执行正向档位。`none` 不属于该序列，只能在固定接口实际包含 `none` 时原样接受，永不转换为正向 effort；
字段缺失与 Responses `reasoning: {}` 保持原样，未知、冲突或非法值仍在 egress 前失败。归一化必须在一次公共接口预检后、Route candidate
展开和 Bridge 转换前执行一次，全部 fallback candidate 获得同一有效档位；随后选中 Upstream API 的 wire mapping 仍独立执行。

Embeddings Native Path 使用独立严格 JSON request union 和有界 JSON response validator；不保留未知字段，不进入 generation
SSE/Bridge，也不在网关转换 vector encoding 或 dimensions。客户端必须以所选
`interfaces.embeddings` 的 forms、domain、parameters 与有效 limits 为准。

显式 `Bridged` Route 必须只转换两协议共同可表达且已由 Upstream API capability 确认可读、方向兼容的 reasoning
channel、text、allowlist Structured Output、function schema、tool call/result identity、非流式 JSON 和流式 SSE lifecycle；
`Unknown`、`Unsupported` reasoning 输出，以及下游请求或 history 中需要继续提交的 opaque continuation，必须拒绝。已完成的上游
Responses 输出转为无状态 Chat response 时，可以在验证 reasoning item 形状后丢弃 Chat 无字段承载的 `encrypted_content`，但不得把它
伪装为 `reasoning_content`，不得丢失同一响应中的可读 summary/content、text 或 tool call，且 JSON/SSE 必须采用同一边界。未知顶层字段、
hosted/custom tool、image、background/store 和其他 Provider 私有扩展必须在 egress 前拒绝。Bridge 不能因字段名
相似、Provider 名称或 capability 并集猜测转换；没有完整 Native/Bridged Route 时返回稳定能力错误。

Responses `reasoning.summary` 的当前公共请求域只包含标准值 `"auto"` 与兼容值 `false`。Native Responses 必须保持客户端原值；
Responses→Chat Bridge 必须接受并消费两者，只把 `reasoning.effort` 转为 `reasoning_effort`，不得向 Chat wire 伪造 summary 开关。
Chat 上游返回的 `reasoning_content` 始终映射为 Responses `reasoning.content[]`/`reasoning_text` JSON 与 SSE lifecycle，`summary` 为空，
不得因下游提交 `"auto"` 而合成 `reasoning.summary[]` 或 `response.reasoning_summary_*`。`false` 只关闭 summary 请求，不关闭
reasoning；`"auto"` 与显式 `effort:"none"` 的冲突、其他 summary string、`true`、`null` 或复合值均在 Provider egress 前失败。

Chat `stream_options` 只允许与 `stream:true` 组合。省略、空对象与 `{"include_usage":false}` 都是合法 no-op：它们不构成能力请求，
并在任何候选 egress 前移除。有效 `{"include_usage":true}` 是必须完整履行的输出契约，只有固定 Public Model Chat interface 明确列出
`stream_options` 时才可执行。Native Chat 必须原样转发有效对象并保留 Provider usage 尾块；Chat→Responses Bridge 必须消费该字段，
从成功 `response.completed.response.usage` 严格投影 prompt/completion/total、cached 与 reasoning token 计数，在 finish 后、`[DONE]`
前生成唯一 `choices:[]` usage-only chunk，并使此前所有 Chat chunk 带 `usage:null`。Bridge 不估算、修正或补造 token；请求 usage 时若
terminal usage 缺失或非法，不得发送 finish、usage-only 或 `[DONE]`。非对象、未知/额外成员、`include_obfuscation`、非布尔
`include_usage` 和非流式组合必须在 Provider egress 前拒绝；Responses interface 继续把该 Chat-only 顶层字段视为未知参数。

流式请求必须满足：

- 原样保持协议的 SSE framing、event/data 负载与输出顺序；不得注入 OpenBridge 自定义 SSE event。
- Chat 以其自身终态（包括 `[DONE]`）处理；Responses 区分 item/content lifecycle 与 `response.completed`、
  `response.incomplete`、`response.failed`、`response.cancelled` 或顶层 `error` 等 response terminal。
- `output_item.done`、tool input delta、metadata/header 到达或任意首字节都不等于请求成功。已写出首个业务 body byte 后，不得
  retry、fallback 或将其他 Upstream Target 的内容拼入当前 stream。
- 下游取消、连接中断、deadline 和错误终态应停止相应上游工作；合法但无 terminal 的 EOF 不得伪造成 completed。
- response headers 和 SSE bytes 的处理必须受大小、UTF-8、event 数量/长度与慢消费者资源上限保护。

上游 API 可以通过可信类型化策略声明自己强制 `stream: true`。这种 API 面对下游非流式请求时只能选择以下一种固定行为：

- 禁用转换：该 Route 对接口贡献 `non_streaming: unsupported`；固定 Public Model 契约按全部候选相交，并在 egress 前拒绝非流式请求，
  不得跳过首选 Route 去选择后续更强候选。
- 启用 Responses SSE buffering：规划器固定写入上游 `stream: true`，在 `max_json_response_body_bytes` 与单 event 上限内完整缓冲，使用
  类型化 Responses lifecycle 校验 framing、identity 和显式 completed/failed/incomplete/cancelled terminal，并从 response snapshot 与
  有序 `response.output_item.done` 组装完整 response，之后才一次性返回 JSON；若下游为 Chat，则再执行既有非流式
  Responses→Chat Bridge。稀疏 terminal 可以补齐已验证的 completed items，但缺失 terminal 不得被补造成成功。

成功响应不是 SSE、非法 UTF-8/framing、body 超限、缺少 terminal、独立 error 或 Bridge 不可表示时必须在下游 body 提交前返回安全的
`invalid_upstream_response`。该开关属于受信 Upstream API 配置，客户端不得覆盖。当前转换只适用于 Responses SSE，不得把 Chat 的
data-only SSE chunks 猜测性聚合为 JSON。

运行期 TTFT 与输出速度必须以实际 token-bearing SSE delta 为边界：除 text 和 function arguments 外，Native wire 中明确出现的
reasoning text delta 也属于生成输出。TTFT、首字节和首输出均只记录第一次命中；后续 chunk/delta 不得重复执行同一聚合热路径。
输出速度的时间窗口从首个上述生成 delta 到原始 upstream body 完成，使 reasoning token 不会进入分子却把其生成时间排除在分母。
该遥测识别不扩大 Public Model reasoning capability，也不授权 Bridge 转换未知 reasoning wire。

## 5. tools、continuation 与扩展

### 5.1 function tools

对于已声明支持的普通 `type: "function"` tool：

- 需要保持请求 schema、并行调用顺序、`call_id` / `tool_call_id`、arguments 分片和 tool result 的关联；
- Responses `input` 中标准 message 可以显式携带 `type: "message"`，也可以使用只包含 `role` 与 `content` 的 shorthand；
  Responses→Chat Bridge 必须对两种写法采用同一 message 转换，缺失 `type` 且含额外字段的模糊对象仍须拒绝；
- arguments 在完成前是未可信的字符串，网关不得执行或授权模型返回的工具调用；
- tool call/result、`item_id`、stream output index 与 request id 是不同身份，不能相互替代。

### 5.2 统一 generation instructions 与无状态核心

通用 Generation 请求按一次解析结果使用同一有效指令：Responses 的显式非空 string `instructions` 优先；Chat 只把
`messages[0]` 中非空纯文本 `system`/`developer` 作为客户端来源；其他情况回落 Bootstrap `default_instructions`。
Responses 显式 `null`、空白或非 string 值在 egress 前返回 400。后续 system/developer 与复合首条消息属于 transcript，
不能扫描、拼接或删除。Chat→Responses 只提升并删除首条合格消息一次，instruction-only 请求必须发送顶层 `instructions`
与 `input: []`；Responses→Chat 把有效值编码为唯一首位 system message。Embeddings 和专用音频 task 不注入。

该值在 Public Model 预检后、candidate 展开前统一写入 canonical request，因此 Native、Bridge、retry、fallback 与 probe
使用同一字节串。`instructions` 是网关 envelope，不属于 canonical Model `supported_parameters`，ChatGPT 或其他 Provider
adapter 不得再次覆盖。

OpenBridge 的核心实现重点是无状态服务；无状态请求是默认使用方式、主要客户端兼容面和当前验收基线。客户端必须在每次请求中
携带所需完整历史，`store` 应省略或为 `false`，`previous_response_id` 应省略或为 `null`，`background` 应省略或为 `false`。
该路径可以在完整能力约束下使用 Native Route、有限 retry/fallback，以及仅转换显式共同语义的 Bridge。

`store:true` 当前不是次要 pass-through：它在所有 Provider egress 前统一拒绝。`previous_response_id` 与 `background` 是次要目标；
当前实现只有能力 gate、状态亲和与唯一签发者校验等基础约束，
尚未形成完整的 response state storage、retrieve/cancel、conversation lifecycle 或 continuation ledger；它们不能作为通用会话、
后台任务或跨 Provider 状态迁移能力使用。正常接入、示例和验收不得以这些字段为前提。

Responses 状态边界固定为：

- `store` 省略与显式 `false` 等价；每个上游 Responses candidate 都显式编码 `store:false`，Responses→Chat Bridge 消费该事实但不伪造 Chat 字段；
- `store:true` 及显式非 boolean/`null` 值稳定失败，不得因某个 Native Provider 接受而扩大；
- 非空 `previous_response_id` 只能原样发送给可由当前配置唯一确定的 issuing Upstream Target/Upstream API； 不能唯一确定时必须在
  egress 前拒绝；
- 有状态请求不得进入 Protocol Bridge 或跨 Upstream Target fallback，不得因 cooldown、权重或暂时故障改投 另一候选；
- OpenBridge 不保存、查询、删除、翻译或迁移上游 response，不承诺 response ID 在 Provider、Target、credential binding
  或部署变更后仍可使用；
- 若未来允许同一 Public Model 的多个 Target 签发可继续使用的 response ID，必须先实现有界、可靠且绑定 issuer 的
  continuation ledger；在此之前不能根据 model 名或 opaque ID 猜测签发者。

### 5.3 状态亲和与私有扩展

- `previous_response_id`、Provider resource、tool continuation、opaque reasoning 与 issuing call 都是可能绑定 Upstream
  Target/Upstream API 的状态。不能安全证明等价时，拒绝、保持同一 issuing target/upstream API，或要求完整可转换历史；不得跨候选猜测或
  replay。
- Codex 所需的 `x-codex-turn-state` 及 `response.metadata` 属于受限私有扩展：只在显式启用的 Codex Native Responses
  profile 中透明保留，不能进入 Bridge IR、用户 transcript、普通日志或跨 target fallback。
- MCP、custom tool、hosted tool、reasoning、annotation、image generation 等不是普通 text 的同义词。所选 Public Model
  固定接口未声明支持时必须在上游调用前拒绝，不得静默丢弃。

### 5.4 普通生成参数的上游兼容

Canonical Model 已声明的普通 Chat/Responses 生成参数可以由下游提交，并继续出现在对应 interface 的
`supported_parameters`。若某个具体 Upstream API 已由官方文档或真实请求确认不接受其中一个参数，代码注册表可以通过闭合、类型化规则
将该字段标记为“下游接受、上游忽略”。选中该 API 后，OpenBridge 必须在 candidate 绑定完成后、进入第一个无法表达该字段的
Bridge/Provider request-shape 转换前静默删除，并保证 transport request 不含该字段；不得因此返回能力错误、改选 Route、改变
fallback 顺序或把固定值伪造成用户请求值。每个 candidate 必须从原始下游 body 独立构造，fallback 选择另一 API 时只应用该 API
自己的规则，不能继承前一个 candidate 已删除的字段集合。

忽略规则必须在启动时验证参数由 canonical Model 声明、集合无重复、且不与 `disabled_parameters` 重叠；它只允许用于 generation API。
首批闭合集合只允许 `frequency_penalty`、`presence_penalty`、`temperature`、`top_p` 和 `seed`。每项规则必须有针对该 Target/API
的明确外部或真实测试证据和确定性 egress 回归测试。未配置为忽略的 Native 普通字段继续保持 wire 语义，不能因为另一个 Provider
不支持就全局删除。

以下字段不属于该兼容例外：`n`、`logprobs`、`top_logprobs`、`include_reasoning`、Responses `include`、prompt-cache 字段、streaming mode、reasoning level/开关、
tools/tool choice、structured output、state/continuation、媒体输入输出、输出 token 上限、认证与 Provider 私有扩展。它们会改变
可观察输出、能力、资源或安全边界；不支持或 Bridge 无法完整表达时必须在 egress 前拒绝，不得静默降级。`supported_parameters` 对普通
忽略字段表示“OpenBridge 接受请求”，不保证每个候选上游都会应用该提示；这一例外不得扩展为任意字符串、用户可配置或请求可选择的过滤器，
也不得提供任意 `extra_body` 绕过类型化目录和固定能力预检。

### 5.5 Responses 输出投影与缓存键转发

- Responses `include` 必须解析为逐值的类型化条件输出请求集合。省略、`null` 与空数组不请求任何值；`include: []` 在一次公共预检后、
  candidate 展开前移除，不能进入 Native 或 Bridge egress。未知 wire 值必须在 egress 前失败关闭。接口接受某个值不保证响应一定出现
  对应 item，也不允许把该值解释为 reasoning 输出存在性或形态开关。
- 每条 Responses Route 只贡献其能安全接受的具体 `include` 值，Public Model 的 `response_includes` 是全部固定候选的集合交集。
  Native 只有在对应 Upstream API 明确接受并能原样转发时才贡献；Bridge 只有在 converter 显式处理该值、保持真实可观察输出且不伪造
  条件 item 时才贡献。没有目标协议 wire 对应物的值必须由 converter 显式消费，不能泄漏到错误的上游协议。
  hosted-tool execution 与 `web_search_call.action.sources` 等输出投影是两个独立能力，请求同时使用时必须共同通过预检。
- `prompt_cache_key` 是请求级转发选项，不是缓存效果能力。它只在全部固定候选都能原样保留时进入 `supported_parameters`；每个 candidate
  必须从同一 canonical body 独立构造并原样转发。OpenBridge 不承诺上游启用缓存、产生命中、降低延迟或成本，也不得以该不确定性为由静默删除键值。
- `prompt_cache_options`、`prompt_cache_retention` 和嵌套 `prompt_cache_breakpoint` 不因缓存键可转发而获得支持；未实现时继续在 egress
  前返回稳定错误。以上字段均不得触发请求期 Route 筛选、跳过或重排。

Responses 标准 event 见[Responses typed SSE 调研](../references/openai/responses/streaming.md)；Codex 私有扩展仍由对应 Codex
项目调研维护。

## 6. 错误与客户端可见结果

| 时机                                        | 必需行为                                                                                                                                              |
|---------------------------------------------|-------------------------------------------------------------------------------------------------------------------------------------------------------|
| ingress、Public Model、能力、认证或配置拒绝 | 上游调用前返回安全、稳定的 OpenAI-compatible JSON error；不暴露 URL、credential、候选列表或内部栈。                                                   |
| Chat/Responses 请求含未知顶层字段            | 上游调用前返回稳定 `unknown_parameter`，`param` 指向安全字段名；Native 与 Bridge 行为一致，且不得调用 Provider。                                       |
| 所选 Public Model 的固定接口契约不支持请求  | 在 egress 前返回稳定 `unsupported_model_capability`；不得改选模型或筛选 Route。只有 5.4 明确定义的普通参数规则可以在选定 API 的 egress 静默删除字段。 |
| 上游在首输出前返回可重试失败                | 该请求已经通过统一能力预检；是否 retry/fallback 只按静态 Route 顺序、错误分类和状态亲和执行，不重新比较候选能力。                                     |
| 首个业务输出前的上游失败                    | 依[路由与 Provider 韧性](provider-resilience.md)判断有限 retry/fallback，最终保留安全的 status、error code、request id 与 allowlist rate-limit 信息。 |
| 已开始 JSON/SSE body 后的失败               | 只使用目标协议已有的 terminal/error 或关闭语义；不重写已发内容、不注入私有 event、不切换 candidate。                                                  |
| 下游取消                                    | 停止当前请求及可取消的 retry/backoff；终态单列为 client cancellation，而非上游成功或错误。                                                            |

所有错误类别必须稳定、低基数且可用于调用统计；原始上游错误正文只能在受保护诊断中按脱敏规则处理，不能成为对外契约。

## 7. 运行期观测与 OpenTelemetry 导出

OpenTelemetry 是可选的 headless 出站观测通道，不是新的下游管理 API。OpenBridge 只负责在协议生命周期边界产生无法从外部重建的
原始事实；collector/backend 负责持久化、窗口查询、分位数、错误率、缓存 token 比例、Provider + Public Model 比较和
可视化。缺失的 Provider usage、cache 或非流式 upstream TTFT 必须保持“未观测”，不得补零或由 gateway body 到达时间伪造。

### 7.1 Signal 所有权

- **Traces**：每个已认证业务请求形成一个 `downstream_request` root span；每个实际出站的 Provider attempt 形成一个有序 child
  span。span 只记录稳定 operation、Public Model、编译期 Provider/Target/Route、Native/Bridged、streaming、低基数 outcome、
  已直接观测的 timing 与 Provider 明确返回的 usage。retry、fallback、取消和 terminal 必须保持实际因果关系且每个 span 只结束一次。
- **Metrics**：只提交用于外部计算的原始 counter/histogram，包括 request/attempt outcome、TTFT、response-ready、duration、
  generation duration、input/output/cache token，以及仅在明确 output usage 和 generation duration 同时存在时计算的单 attempt
  output speed。metric attributes 只允许有界的 Provider、Public Model、upstream model、typed operation、Route/Target、Route mode、
  streaming 和 outcome；request id、trace id、user、HTTP status 原值、错误文本或 endpoint URL 不得成为 metric attribute。平均值、
  分位数、cache ratio、error rate 或 Provider 排名由外部系统计算。
- **Logs**：导出启动、关闭、exporter 状态和需要人工诊断的安全结构化事件，并通过 trace/span id 关联业务 trace。不得为每个 SSE
  chunk/delta 产生日志，也不得把已经由 attempt/request span terminal 完整表达的事实再复制为一组高频业务日志。
- **本地开发内容日志**：Bootstrap 的四个独立开关可以分别记录认证后下游 request header/body 与最终 response header/body；
  仓库随附开发配置显式全开，自定义配置缺表或缺字段时对应回退关闭。
  header 值先强制脱敏认证、Cookie 和 secret-like 名称；body 只保留既有 request/JSON-response budget 内的有界 snapshot，长流明确
  标记截断且每个方向最多一个事件。该本地 formatter 事件不进入 span-only OTLP layer，不得被解释为原始 Provider wire dump。

OpenBridge 不执行下游 Agent 的工具，不能从 tool arguments、tool result 文本或下一轮 prompt 猜测工具是否执行成功；实际 tool error
rate 只有在未来存在显式、低基数且不携带业务内容的客户端 outcome 契约时才可统计。本次迁移只保留已有的协议级 tool 生命周期事实，
不为获取工具错误率增加正文解析或日志采集。

### 7.2 配置、安全与运行时边界

- exporter 默认禁用，只能由 bootstrap 显式启用并提供受限 URL shape 的 OTLP/HTTP collector；配置所有者可以选择 loopback、非
  loopback IP 或 DNS host，业务请求不能选择 endpoint、protocol、header、resource attribute 或采样策略。无效 scheme、缺失 host、
  URL credential、path、query、fragment 或不支持字段必须在 listener 与 exporter egress 前失败。
- 所有 signals 使用固定 `service.name = "openbridge"` 和本次进程资源身份。traces 可携带 request id 以供关联；任何 signal 都不得包含
  Authorization、credential、用户身份、请求/响应正文、tool arguments/result、reasoning 正文、原始上游错误正文、query 或真实
  endpoint URL。
- 本地开发内容日志不是 OTLP signal；即使显式启用，认证、Cookie、credential 与 secret-like header 值仍不得进入日志。body snapshot
  可能包含受控开发业务内容；随附开发配置显式全开，生产部署必须由 bootstrap 所有者按需关闭。
- request hot path 只写入内存中的有界 signal primitive；网络 export 必须批处理并与请求异步隔离。队列满、collector 不可达或 export
  timeout 只能丢弃观测并产生有界、限频的本地诊断，不能改变下游状态、重试、fallback、取消或 Provider 结果。关闭时 flush 也必须有界。
- metrics 只通过启动时配置的 OTLP/HTTP 出站；不得保留自定义进程内累计查询 API，旧
  `/openbridge/v1/metrics` 与 `/openbridge/v1/metrics/providers` 必须保持未注册。
- OpenBridge 不内置 collector、SQLite、历史数据库、dashboard、Prometheus endpoint 或分布式聚合；这些属于外部部署和分析程序。

## 8. 功能验收要求

| ID     | 应被保护的用户可观察行为                                                                                                                                                           |
|--------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| API-01 | 有效静态 token 可访问标准/扩展模型与业务 endpoint；认证失败、未知 Public Model、不支持 feature 与非 JSON 请求在 egress 前安全失败。                                                |
| API-02 | 标准/扩展 Models 接口满足[模型能力契约](model-information-and-capability-contract.md)的身份、逐字段一致性与部署信息隔离要求。                                                      |
| API-03 | Native Chat/Responses 已知且被接口接受的请求字段除统一 instructions/store envelope、固定 Public Model reasoning 输入归一化、受信模型/认证改写、Provider wire mapping 及已验证的普通参数忽略规则外保持 wire 语义；未知请求顶层字段在 egress 前拒绝，上游响应中的未知合法字段/event 不因网关丢失。 |
| API-04 | SSE 分片、终态、EOF、上游 error 和下游 cancel 不会产生伪成功、重复 terminal 或跨 Upstream Target 拼接。                                                                            |
| API-05 | Chat/Responses 普通 function tool 的 call/result identity 与 fragmented arguments 在已声明路径中保持；生成链路不执行这些工具。                                                      |
| API-06 | Codex Native profile 能在受限 allowlist 下保留其已验证的 turn-state 扩展；bridge、route change 或 fallback 不会误复用该状态。                                                      |
| API-07 | 对 Codex、OpenAI SDK 或 Hermes 的兼容声明均有相应 endpoint/feature 的可重复证据，并写入实施现状而非仅引用设计。                                                                    |
| API-08 | 客户端只选择 Public Model 与下游协议；固定能力契约不支持时统一拒绝，普通忽略参数按选中 API 删除，其他支持请求保持配置 Route 顺序，不按请求能力筛选或重排候选。                       |
| API-09 | 无状态请求避开短时 cooldown 的 quota/fault scope；target-bound continuation 不因健康状态切换 issuing target。                                                                      |
| API-10 | reasoning input 只接受 canonical vocabulary 与 Public Model `accepted_levels` 的交集；`strict` 保持精确值，`clamp_positive_floor` 只在正向 effort 中解析到固定接口实际 `levels`，`none` 不参与转换；有效值再按选定 Upstream API 的已校验规则改写，未知值、歧义源或非法目标在 egress 前失败。   |
| API-11 | 无状态 Responses 是核心兼容面、默认使用方式和当前验收基线；`store` 省略或 false 均规范化为每个 Responses egress 的显式 false，true 在 egress 前拒绝；非空 `previous_response_id` 与 `background:true` 仍属次要且不完整的 Native 目标。 |
| API-12 | Embeddings、图片、文件与音频分别满足[扩展共同规则](embedding-and-native-multimodal.md)及其功能页的 wire、能力、资源归属、限制和证据边界。                                             |
| API-13 | token-bearing text/tool/reasoning SSE delta 只触发一次 TTFT/生成窗口，非流式 Chat/Responses 成功 JSON 只在首个非空下游 body chunk 记录一次可直接观测的 gateway TTFT，不得据此伪造 upstream TTFT、生成时长或输出速度；OTLP metrics 不含请求正文、响应正文、Authorization、credential、用户或 request ID。 |
| API-14 | 有效静态 token 可通过 `POST /mcp` 发现唯一静态 `hello(name: string)` 工具并取得 `Hi, {name}!`；Origin、transport metadata、无效参数、未知工具/method 与非 POST method 按固定边界失败，且调用不访问 Provider 或外部系统。 |
| API-15 | `include: []` 作为 no-op 在全部 egress 前移除；非空 `include` 按 Public Model 逐值交集预检，未知或 Bridge 不可保真的投影 zero-egress 失败；`prompt_cache_key` 只在固定候选全部支持时原样转发，且不承诺缓存效果。 |
| API-16 | Chat `stream:true` 下的空 `stream_options` 与 `include_usage:false` 作为 no-op 在能力预检和 egress 前移除；有效 `include_usage:true` 只有在固定 Chat interface 完整保证时接受，Native 原样保留，Chat→Responses Bridge 从合法 terminal usage 生成标准 usage-only 尾块，非法形状、Responses 顶层字段和缺失/非法 terminal usage 均 fail closed。 |
| API-17 | 通用 Generation 只解析一次客户端 instructions 来源并在缺失时使用项目默认值；Native/Bridge/候选/重试/probe 编码一致，首条合格 Chat 指令只提升删除一次，后续 transcript 保序，专用 task 不注入。 |
| API-18 | Responses `reasoning.summary` 接受 `"auto"` 与兼容 `false`：Native 精确保留，Responses→Chat 消费且只返回真实 Chat `reasoning_content` 对应的 Responses reasoning content，不伪造 summary；非法值与 `none+auto` 在 egress 前失败。 |
| OBS-01 | OTLP exporter 默认禁用；只有合法的 startup-only OTLP/HTTP 配置能启用相应 signal，collector host 可由配置所有者选择，非法配置在 listener 和 exporter egress 前失败，业务请求无法覆盖。 |
| OBS-02 | 一个已认证业务请求产生一个脱敏 request root span，每个实际 Provider attempt 产生一个有序 child span；terminal、retry、fallback、失败与取消不重复也不改变实际因果关系。       |
| OBS-03 | OTLP metrics 使用 SDK 原生 counter/histogram 和有界维度；单 attempt output speed 只由明确 output usage 与 generation duration 计算，分位数、平均值、错误率、缓存 token 比例与 Provider + Public Model 排名由外部系统计算，未知值不补零。 |
| OBS-04 | OTLP logs 只导出安全、限频且可通过 trace/span id 关联的运行诊断；不记录逐 chunk/delta，也不复制完整 request/attempt terminal 形成冗余高频日志。                              |
| OBS-05 | export 使用有界异步队列和有界关闭；collector 故障、超时或背压不阻塞请求、不改变 HTTP/SSE/Provider 行为，只允许丢弃 telemetry 并产生限频本地诊断。                          |
| OBS-06 | 所有 signals 都不包含 credential、Authorization、用户身份、业务正文、tool/reasoning 内容、原始错误正文、query 或真实 endpoint URL；metric attributes 不含高基数身份。       |
| OBS-07 | OTLP metrics 覆盖 request/attempt、韧性、timing、usage 与 cache 事实后，旧 metrics HTTP endpoint 和自定义 snapshot 聚合保持删除，不为未发布原型保留兼容垫片。 |
| OBS-08 | 本地下游 HTTP header/body 日志由四个彼此独立的 bootstrap 开关控制；随附开发配置显式全开、缺表/缺字段时回退关闭，只覆盖认证后客户端边界，敏感 header 强制脱敏、body capture 有界且每个方向最多一个终态事件，并保持 OTLP exclusion。 |

## 9. 非目标

- GUI、Web 控制台、客户端安装/注册/配置管理；
- Realtime、Responses WebSocket、Files、Images、Videos、Conversations、管理 API 或“实现全部 OpenAI API”；
- 保存、查询、删除、翻译或跨 Provider/Target 迁移 response 状态，以及未有真实需求前实现 continuation ledger；
- 让 Chat ↔ Responses、任何 tool 或 Provider 私有扩展自动无损互转；
- 代表下游 Agent 执行任意 function tool、shell、computer 或网页操作；
- 在 MCP endpoint 中执行 `hello` 以外的工具、桥接 Provider、产生外部 side effect、兼容旧版 session lifecycle 或提供浏览器 Origin allowlist；
- 用 API token 建立多用户权限、配额、账单或审计系统。

## 关联文档

- [产品范围](product-scope.md)
- [Public Model 与模型能力契约](model-information-and-capability-contract.md)
- [配置与凭证](configuration-and-credentials.md)
- [路由与 Provider 韧性](provider-resilience.md)
- [交付与证据要求](delivery-and-evidence.md)
- [MCP 2026-07-28 外部协议与 Rust 生态调研](../references/mcp/README.md)
- [当前实现总览](../implementation-status/current-implementation.md)
