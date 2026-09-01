# 网关 API 合同

本文集中定义 HTTP/MCP 入口、认证、Generation 请求与响应、streaming、tool、state、错误和验收约束。

受信用户把本地 Agent 或 OpenAI-compatible SDK 指向稳定 base URL，使用私有用户表中的 Bearer API key 和
Public Model 调用服务；客户端不需要、也不能选择上游 Provider、真实模型、URL、credential 或候选切换。

当前合同覆盖 OpenAI-compatible Chat Completions、Responses、Embeddings 与 Images HTTP JSON/SSE，同协议 Native
媒体、按任务分离的 Chat Native audio、显式共同语义内的 Chat/Responses Bridge，以及 MCP stateless/legacy lifecycle。
“请求可转发”不等于“某个 Agent 完整兼容”；客户端 profile、transport 和 tool loop 只有在本文明确声明并有对应证据时才构成承诺。

## Endpoint 与认证

本文定义下游可调用的 endpoint 集合与认证边界。MCP 的协议生命周期由
[MCP 本地服务](gateway-api.md#mcp-本地服务)单独拥有。

### 1. Endpoint 总览

| 接口 | 功能要求 | 不包含的语义 |
|---|---|---|
| `GET /healthz` | 提供不访问上游 credential 的最小本地存活信息，不泄露 Route、Target 或 secret。 | Provider 健康 probe、控制面或客户端管理。 |
| `GET /v1/models`、`GET /v1/models/{model}` | 按[模型能力契约](model-capability.md)返回严格 OpenAI 四字段 list/retrieve。 | 扩展能力、上游模型或部署信息。 |
| `GET /openbridge/v1/models`、`GET /openbridge/v1/models/{model}` | 返回同一 Public Model 目录的模型事实和固定接口契约。 | Provider/Target/Route、credential、健康、价格或动态发现。 |
| `POST /v1/chat/completions` | 在固定 Chat interface 内提供 JSON/SSE 与已声明的 Native 媒体能力。 | 多模态 Bridge、未声明 audio output、专用资源 API 或 hosted tool 默认兼容。 |
| `POST /v1/responses` | 在固定 Responses interface 内提供 JSON/SSE 与已声明的 Native 媒体输入。 | 多模态 Bridge、Responses audio/WebSocket、response 资源与 background API。 |
| `POST /v1/embeddings` | 提供独立 Embedding Public Model 的有界 JSON 请求/响应。 | streaming、向量转换/存储/检索或无 identity 证明的 fallback。 |
| `/mcp` | 按 [MCP 本地服务](gateway-api.md#mcp-本地服务)提供 stateless 与 legacy session 两种 Streamable HTTP lifecycle。 | 动态 tool、Provider Bridge、外部 side effect、resource 或 prompt。 |

### 2. 认证边界

- Models、generation、Embeddings 与 MCP endpoint 使用私有用户表分配的静态 Bearer API Key。
- 用户表只在启动时读取；不提供在线 key issuance、scope、即时撤销、配额或 billing identity，变更需要重启。
- 认证失败必须在模型查询、JSON-RPC dispatch、Route planning 或 Provider egress 前结束。
- 未认证错误不得泄露用户、registry、credential、endpoint、Route 或 MCP session 是否存在。
- `GET /healthz`、OpenAPI 与 Swagger UI 是公开资源，但只能暴露其各自的静态最小信息。

## 请求与安全边界

### 1. Public Model 与 routes

- 下游只能提供已配置的 Public Model；它表示 OpenBridge 对下游提供的稳定服务契约，而不是某个上游模型名的透明别名。身份、生命周期、固定能力计算、Models
  API 和错误语义统一由[Public Model 与模型能力契约](model-capability.md)定义。
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

### 2. 输入保护

- 仅接受端点契约允许的 content type、JSON body 和受配置约束的大小；无法安全解析的请求在 egress 前返回稳定错误。
- 请求分类必须先识别 operation，再按 operation 解析 `stream`、input form、function/custom/hosted
  tool、并行工具、结构化输出、multimodal、reasoning、`previous_response_id`、background/store 与相应限制等会影响固定契约或状态边界的特征。
- Chat/Responses 下游请求的顶层字段必须先按源协议的代码内类型化目录分类；未知字段即使值为 `null` 也必须在 egress 前以稳定
  `unknown_parameter` 拒绝，不能因"目标 Provider 也许支持"而进入 Native 或 Bridge。已知字段的 `null`/`false` 是否表示未请求能力，
  只由该字段的类型化语义决定，不形成通用绕过规则。
- 服务为每个请求生成或传播安全的 request id，用于响应和受控诊断；该 id 不是 client identity、tool identity 或聚合指标
  label。

## Native Path 与流式语义

### 1. Native Path 基线

当下游与上游协议一致且请求已通过 Public Model 固定契约预检与输入归一化时，Native Path 是兼容性基线：它只做受信路由、模型、认证、显式
reasoning level wire 映射和已验证的普通生成提示忽略，保留其他已知且被接口接受的请求 JSON，并保持上游响应中的未知合法 JSON
字段/SSE event，不经过通用 IR 重渲染。level 映射必须属于选定 Upstream API 的代码注册规则，映射源必须已由 canonical Model
声明，目标必须是安全 wire 值；不得由业务请求提供映射或用映射扩大 Public Model 支持的下游 level 集合。canonical reasoning
level vocabulary 为 `none`、`minimal`、`low`、`medium`、`high`、`xhigh`、`max`；每个 Model 仍须显式声明实际支持的子集。`none` 是调用方显式要求禁用
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
- 上游 Chat JSON/SSE usage 的 `completion_tokens_details` 与 `prompt_tokens_details` 省略或显式 `null` 都表示对应 detail absent；对象时只读取已建模 token 字段，其他值继续 fail closed。Native 验证后仍保留原始 response bytes，不把 `null` 改写为空对象。

### 2. 流式语义

流式请求必须满足：

- 原样保持协议的 SSE framing、event/data 负载与输出顺序；不得注入 OpenBridge 自定义 SSE event。
- Chat 以其自身终态（包括 `[DONE]`）处理；Responses 区分 item/content lifecycle 与 `response.completed`、
  `response.incomplete`、`response.failed`、`response.cancelled` 或顶层 `error` 等 response terminal。
- `output_item.done`、tool input delta、metadata/header 到达或任意首字节都不等于请求成功。已写出首个业务 body byte 后，不得
  retry、fallback 或将其他 Upstream Target 的内容拼入当前 stream。
- 成功 headers 后、第一个完整合法且下游可见的 SSE event 前仍未 commit。first-event timeout 或 body transport failure 可按既有有限 attempt policy
  retry/fallback；首 frame invalid 或 terminal 前 clean EOF 必须在零 downstream event 时返回安全 502，且不得伪装成可重放 transport failure。
- 第一个合法且下游可见的 event 到达后才 commit 200/SSE；Native 首先下发该已验证的原始 event，Bridge 首先下发其确定性转换输出。commit 后 transport error 或 terminal 前 clean EOF 必须
  保留已发送 bytes、以 body error 结束，禁止 retry/fallback、拼接第二条流或合成 `completed`/`failed`/`[DONE]`。
- 下游取消、连接中断、deadline 和错误终态应停止相应上游工作；合法 terminal 后的普通 close 不得反转已确认终态。
- 上游非流式响应的 total deadline 与 SSE 生命周期必须分开表达。SSE 必须分别约束等待 response headers、等待首个有效 event、
  event 间 idle 与可选的 stream total safety deadline；普通非流式 total deadline 不得从连接开始持续覆盖一条仍在合法产生 event 的 stream。
- timeout policy 只能来自受信 Target/API 与实际 upstream delivery mode，客户端不得覆盖。关闭 streaming total deadline 时仍必须保留
  bounded headers/first-event/idle policy；不得以修复长流截断为由把所有等待改成无限。
- response headers 和 SSE bytes 的处理必须受大小、UTF-8、event 数量/长度与慢消费者资源上限保护。
- precommit raw buffer 最多保存一个 `max_sse_event` 约束的 event。Bridge 遇到转换后不可见的合法 event 时必须推进并
  hand off 同一个 renderer state、立即释放该 event 的 raw bytes；不得把多个 event 累积成 prefix，也不得重新渲染已消费 event。

上游 API 可以通过可信类型化策略声明自己强制 `stream: true`。这种 API 面对下游非流式请求时只能选择以下一种固定行为：

- 禁用转换：该 Route 对接口贡献 `non_streaming: unsupported`；固定 Public Model 契约按全部候选相交，并在 egress 前拒绝非流式请求，
  不得跳过首选 Route 去选择后续更强候选。
- 启用 Responses SSE buffering：规划器固定写入上游 `stream: true`，在 `max_json_response_body` 与单 event 上限内完整缓冲，使用
  类型化 Responses lifecycle 校验 framing、identity 和显式 completed/failed/incomplete/cancelled terminal，并从 response snapshot 与
  有序 `response.output_item.done` 组装完整 response，之后才一次性返回 JSON；若下游为 Chat，则再执行既有非流式
  Responses→Chat Bridge。稀疏 terminal 可以补齐已验证的 completed items，但缺失 terminal 不得被补造成成功。

成功响应不是 SSE、非法 UTF-8/framing、body 超限、缺少 terminal、独立 error 或 Bridge 不可表示时必须在下游 body 提交前返回安全的
`invalid_upstream_response`。该开关属于受信 Upstream API 配置，客户端不得覆盖。当前转换只适用于 Responses SSE，不得把 Chat 的
data-only SSE chunks 猜测性聚合为 JSON。

### 3. 遥测计时边界

运行期 TTFT 与输出速度必须以实际 token-bearing SSE delta 为边界：除 text 和 function arguments 外，Native wire 中明确出现的
reasoning text delta 也属于生成输出。TTFT、首字节和首输出均只记录第一次命中；后续 chunk/delta 不得重复执行同一聚合热路径。
输出速度的时间窗口从首个上述生成 delta 到原始 upstream body 完成，使 reasoning token 不会进入分子却把其生成时间排除在分母。
该遥测识别不扩大 Public Model reasoning capability，也不授权 Bridge 转换未知 reasoning wire。

## 参数兼容

本文拥有普通 generation 参数的受控上游忽略规则，以及 Responses `include` 和 prompt-cache 请求字段。

### 1. 普通 generation 参数

Canonical Model 声明的普通 Chat/Responses 参数可以由下游提交，并继续出现在对应 interface 的
`supported_parameters`。某个具体 Upstream API 明确不接受其中一个参数时，代码注册表可以通过闭合、类型化
规则将其标记为“下游接受、该上游忽略”。

选中该 API 后，OpenBridge 必须在 candidate 绑定完成后、进入首个无法表达该字段的 Bridge/Provider request
转换前静默删除，并保证 transport request 不含该字段。删除不得返回能力错误、改选 Route、改变 fallback 顺序
或伪造固定值。每个 candidate 都从同一 canonical downstream body 独立构造，不能继承前一 candidate 的删除结果。

忽略规则必须满足：

- 参数由 canonical Model 声明；
- 集合无重复，且不与 `disabled_parameters` 重叠；
- 只用于 generation API；
- 闭合集合只包含 `frequency_penalty`、`presence_penalty`、`temperature`、`top_p` 与 `seed`；
- 未配置为忽略的 Native 普通字段保持 wire 语义，不能因为另一 Provider 不接受就全局删除。

以下字段不属于该例外：`n`、`logprobs`、`top_logprobs`、`include_reasoning`、Responses `include`、
prompt-cache 字段、streaming mode、reasoning、tool/tool choice、Structured Output、state/continuation、媒体、
输出 token 限制、认证与 Provider 私有扩展。它们改变可观察输出、能力、资源或安全边界；不支持时必须拒绝，
不得静默降级。

对普通忽略字段，`supported_parameters` 只表示 OpenBridge 接受该请求，不保证每个候选上游都会应用该提示。
规则不得扩展为任意字符串、用户配置或请求可选过滤器。

### 2. Responses `include`

- `include` 解析为逐值的类型化条件输出请求。省略、`null` 与空数组不请求任何值；`include: []` 在一次公共
  预检后、candidate 展开前移除。未知 wire 值在 egress 前拒绝。
- 每条 Responses Route 只贡献能安全接受的具体值，Public Model 的 `response_includes` 是全部固定候选的
  公共 accepted set 交集；candidate 的私有 forwarded set 不得通过 Models API 泄漏。
- 除下述精确例外外，Native 只有在 Upstream API 原样接受时才贡献；Bridge 只有在 converter 显式消费或
  转换该值、保持真实可观察输出且不伪造 item 时才贡献。
- `reasoning.encrypted_content` 是当前唯一批准的 request compatibility hint。所有固定 Responses Route 都可安全
  接受：Native candidate 原生支持时原样转发，不支持时只删除该元素；Responses→Chat candidate 也由 planning
  在进入 Bridge 前删除该元素，converter 不再拥有该 hint 的第二套消费规则，任何意外残留的 active `include`
  必须 fail closed。删除后数组为空时删除顶层 `include`。该规则不得扩展到其他 include 值，也不得筛选、跳过或重排 candidate。
- 接受某个值不保证 response 一定出现对应 item，也不表示 hosted-tool execution 或 reasoning 输出形态得到
  额外支持；删除 hint 时不得合成 output item，也不表示 opaque encrypted content 可以跨 issuer、credential、
  Target 或 Provider 重放。

### 3. Prompt-cache 与 parallel-tool 控制

- `prompt_cache_key` 是 best-effort 请求 hint，不是缓存效果能力。Chat/Responses 固定 interface 接受 string；省略或
  `null` 在 candidate 展开前移除。具体 Upstream API 原生支持时精确转发，不支持时只对该 candidate 删除。
- `prompt_cache_key` 的接受合同进入 `supported_parameters`，candidate 是否精确转发保持私有；OpenBridge 不承诺
  cache hit、延迟或成本变化，也不得用该 hint 选择、跳过或重排 Route。
- `prompt_cache_retention` 省略或 `null` 等价。当前只识别 `in_memory` 与 `24h` 的合法 shape，但非空请求仍作为
  未实现能力 zero-egress 拒绝；不得静默删除、猜测映射到 `prompt_cache_options.ttl` 或把 Provider 默认保留策略
  解释为已满足。`prompt_cache_options` 与嵌套 `prompt_cache_breakpoint` 也不因 key 可接受而获得支持。
- `parallel_tool_calls` 只在 function tool 可执行时是 active control。没有 function tool 或 `tool_choice:"none"` 时，
  合法 boolean 值在 candidate 展开前移除；Responses `null` 也等价于省略，Chat `null` 与非 boolean shape 拒绝。
- active `parallel_tool_calls:true` 只有固定 interface 能精确控制并行工具调用时才接受并保留；active `false`
  可以由同一 toggleable wire 精确转发，或由每个 candidate 显式注册的 serial-only contract 保证并在 egress 删除。
  未知或未证明 control 继续返回 `unsupported_model_capability`；不得为减少 400 删除 active control 或从
  `parallel_calls:false` 猜测上游必然串行。

## Generation envelope 与状态

本文拥有 Chat/Responses 的统一 instructions、无状态默认、Responses state 字段与 issuer affinity 契约。

### 1. 统一 instructions

通用 Generation 请求只解析一次有效指令来源：

- Responses 的显式非空 string `instructions` 优先；`null`、空白或非 string 值返回 400。
- Chat 只把 `messages[0]` 中非空纯文本 `system`/`developer` 作为客户端来源；后续 system/developer 与
  复合首条消息都属于 transcript，不能扫描、拼接或删除。
- 没有客户端来源时使用 Bootstrap `default_instructions`。
- Chat-to-Responses 只提升并删除首条合格消息一次；instruction-only 请求发送顶层 `instructions` 与
  `input: []`。Responses-to-Chat 把有效值编码为唯一首位 system message。
- Embeddings 与专用音频 task 不注入通用 instructions。

有效值在 Public Model 预检后、candidate 展开前写入 canonical request；Native、Bridge、retry、fallback 与
probe 必须使用同一值。`instructions` 是 gateway envelope，不属于 canonical Model `supported_parameters`，
Provider adapter 不得再次覆盖。

### 2. 无状态默认

客户端默认携带完整历史，并使用：

- `store` 省略或为 `false`；
- `previous_response_id` 省略或为 `null`；
- `background` 省略或为 `false`。

该路径可以在固定能力契约内使用 Native Route、有限 retry/fallback，以及只转换共同语义的 Bridge。

当前 allow/deny 行为固定为：

- `store:true`、`store:null` 及其他非布尔显式值稳定失败；每个 Native Responses candidate 显式编码
  `store:false`，Responses-to-Chat Bridge 消费该事实而不向 Chat wire 添加字段。
- `background:false` 或省略表示同步请求；当前 Public Model interface 不公开 background capability，
  `background:true` 在 Provider egress 前拒绝。
- `previous_response_id:null` 或省略表示无 continuation；当前 Public Model interface 不公开 response-ID
  continuation，任何非 `null` 值都在 Provider egress 前拒绝。
- OpenBridge 不提供 response store/retrieve/cancel/delete、background job、conversation lifecycle 或
  continuation ledger。客户端不得把这些字段当作通用会话或后台任务能力。

### 3. State affinity 安全不变量

如果后续需求扩大固定接口，仍必须同时满足：

- 非空 `previous_response_id` 只能原样发送给由固定接口唯一确定的 issuing Upstream Target/Upstream API；
  `TargetBoundContinuation` 是唯一可以贡献该能力的 executable state。
- 多个潜在 issuer、Bridge、跨 Target fallback 或缺少 credential affinity 时，固定接口必须关闭 continuation。
- 有状态请求不得因 cooldown、权重、暂时故障或相同模型名改投另一 Target。
- OpenBridge 不保存、翻译或迁移 Provider state，不承诺 response ID 在 credential generation、Target binding
  或部署变更后仍可使用。
- Provider resource、tool continuation、opaque reasoning 与 issuing call 都可能绑定 Target/API；不能证明
  等价时必须拒绝或要求完整可转换历史，不得猜测或 replay。

这些不变量不表示当前已经开放 continuation；当前公开契约仍按第 2 节拒绝。

## Function tool 与扩展

本文拥有普通 function-tool identity、生成链路的不执行边界，以及受限 Provider 私有扩展。

### 1. Function tools

对固定接口声明支持的普通 `type: "function"` tool：

- 保持请求 schema、tool choice、并行调用语义、`call_id`/`tool_call_id`、arguments 分片与 tool result 关联；
- Responses `input` 中标准 message 可以显式携带 `type: "message"`，也可以使用只含 `role` 与 `content`
  的 shorthand；Responses-to-Chat Bridge 对两种写法使用同一转换，缺少 `type` 且有额外字段的模糊对象拒绝；
- arguments 在完成前是不可信字符串，OpenBridge 不执行或授权模型返回的 tool call；
- tool call/result、`item_id`、stream output index 与 request id 是不同 identity，不得相互替代；
- Bridge 只转换两端都能完整表达的 function schema、choice、call/result identity 与 lifecycle，不能丢弃字段、
  修复 arguments 或根据 Provider 名称猜测语义。

### 2. 私有扩展

- Codex 的 `x-codex-turn-state` 与 `response.metadata` 只可在显式启用的 Codex Native Responses profile 中
  透明保留，不能进入 Bridge IR、普通 transcript、业务日志或跨 Target fallback。
- opaque continuation 或 encrypted reasoning 不是普通 text。若目标协议没有等价 wire，必须拒绝或按明确的
  无状态完成响应规则丢弃不可继续提交的 opaque 内容；不得转换成明文 reasoning。
- custom tool、hosted tool、MCP、annotation、image generation 与 Provider 私有字段都不是普通 function tool
  的别名。固定 interface 未声明支持时必须在 egress 前拒绝，不得静默删除。
- 业务请求不能通过 `extra_body`、任意 header 或未建模 tool type 绕过 Public Model 固定能力预检。

### 3. MCP 隔离

[MCP 本地服务](gateway-api.md#mcp-本地服务)只执行静态 `hello`，与 generation function-tool wire 互不调用。MCP tool catalog 不进入
Public Model 能力；generation tool call 也不会被发送到 `/mcp` 执行。

## 错误与客户端结果

### 1. 错误时机与必需行为

| 时机                                        | 必需行为                                                                                                                                              |
|---------------------------------------------|-------------------------------------------------------------------------------------------------------------------------------------------------------|
| ingress、Public Model、能力、认证或配置拒绝 | 上游调用前返回安全、稳定的 OpenAI-compatible JSON error；不暴露 URL、credential、候选列表或内部栈。                                                   |
| Chat/Responses 请求含未知顶层字段            | 上游调用前返回稳定 `unknown_parameter`，`param` 指向安全字段名；Native 与 Bridge 行为一致，且不得调用 Provider。                                       |
| 所选 Public Model 的固定接口契约不支持请求  | 在 egress 前返回稳定 `unsupported_model_capability`；Generation 的 `param` 必须定位到标准顶层字段。不得改选模型或筛选 Route。只有普通参数兼容规则明确定义的参数可以在选定 API 的 egress 静默删除字段。 |
| 上游在首输出前返回可重试失败                | 该请求已经通过统一能力预检；是否 retry/fallback 只按静态 Route 顺序、错误分类和状态亲和执行，不重新比较候选能力。                                     |
| 首个业务输出前的上游失败                    | 依[路由与 Provider 韧性](routing-resilience.md)判断有限 retry/fallback，最终保留安全的 status、error code、request id 与 allowlist rate-limit 信息。 |
| 已开始 JSON/SSE body 后的失败               | 只使用目标协议已有的 terminal/error 或关闭语义；不重写已发内容、不注入私有 event、不切换 candidate。                                                  |
| 下游取消                                    | 停止当前请求及可取消的 retry/backoff；终态单列为 client cancellation，而非上游成功或错误。                                                            |

所有错误类别必须稳定、低基数且可用于调用统计；原始上游错误正文只能在受保护诊断中按脱敏规则处理，不能成为对外契约。

### 2. Generation 字段定位与首错

Chat/Responses 的合法字段超出固定 Public Model interface 时，HTTP status、`error.type`、`error.code` 和固定 message 保持不变，
`error.param` 使用下列标准顶层 owner；内部 capability reason 不进入下游响应：

| 失败事实 | `param` |
|---|---|
| streaming 或 non-streaming delivery | `stream` |
| function tool / strict schema | `tools` |
| tool choice | `tool_choice` |
| parallel function calls | `parallel_tool_calls` |
| Chat / Responses structured output | `response_format` / `text` |
| continuation、background、Responses projection | `previous_response_id`、`background`、`include` |
| Chat / Responses multimodal input | `messages` / `input`；独立音频控制使用 `audio` 或 `asr_options` |
| output limit | 实际触发限制的 `max_output_tokens`、`max_completion_tokens` 或 `max_tokens` |
| Chat / Responses reasoning | `reasoning_effort` / `reasoning` |
| ordinary interface parameter | 该字段本身 |

已知字段形状非法继续返回 `invalid_request_error`；未知字段继续返回 `unknown_parameter`，不能误报为 capability。一个响应只返回一个错误，
顺序固定为：JSON envelope/model、unknown field、shape/combination、Public Model/interface、stream、tools/tool choice/parallel/strict、
structured output、state、`include`、multimodal、output limit、reasoning、ordinary parameter。JSON key、集合与 candidate 顺序不得改变首错；
所有本地拒绝必须保持 zero egress，且不得回显字段值或执行拓扑。

## MCP 本地服务

MCP endpoint 与 Chat/Responses 中的 function-tool wire 转发相互独立。它只提供显式注册的本地 tool，不把
Public Model、Provider、Target、Route 或上游 credential 暴露为 MCP tool。

### 1. Dual-era transport contract

- `/mcp` 使用与业务 API 相同的静态 Bearer 认证。所有 browser `Origin` 一律在认证与 JSON-RPC dispatch 前
  以 HTTP `403` 拒绝；本服务没有 browser Origin allowlist。
- `2026-07-28` 客户端使用无状态 `server/discover` 路径；每个 POST 请求自带完整 protocol、client 与
  capability metadata，不创建 session。
- legacy 客户端使用 `initialize`/`initialized` handshake，并由同一 `/mcp` endpoint 管理
  `Mcp-Session-Id`、GET SSE stream 与 DELETE session lifecycle。legacy session compatibility 是当前契约，
  不是非目标。
- 两种 lifecycle 都必须发现同一个静态 tool catalog，并受相同的认证、Origin、request body、request id、
  敏感 header 与终态观测边界保护。
- `server/discover` 必须声明 `tools` capability 并返回支持的 protocol version 列表；version negotiation 失败
  必须返回稳定 JSON-RPC error，不能猜测或降级到未声明协议。

### 2. Stateless metadata

`2026-07-28` 请求必须：

- 使用 `POST /mcp` 和 `application/json` body，并同时接受 `application/json` 与 `text/event-stream`；
- 携带 `MCP-Protocol-Version` 与 `Mcp-Method`，并与 JSON-RPC body 和 `_meta` 中的 protocol version、method、
  client info/capabilities 一致；
- 对 `tools/call` 携带与 body tool name 一致的 `Mcp-Name`。

缺失、畸形或不一致的 metadata 必须在 tool 执行前失败。该 header contract 不得被误用于拒绝合法 legacy
initialize/session lifecycle。

### 3. `hello` tool

- `tools/list` 按确定性顺序返回唯一的 `hello`；其 closed `inputSchema` 只接受一个必需字符串 `name`。
- 有效 `tools/call` 返回一个 text content block：`Hi, {name}!`。
- `hello` 不读取配置、registry、文件、网络或 Provider，也不产生外部 side effect。
- 无效 argument 返回 `isError: true` 的 tool result；未知 tool 返回 JSON-RPC `-32602`。
- 未实现 JSON-RPC method 返回 `-32601`；认证、Origin、session 和 transport 错误保持各自 HTTP/JSON-RPC
  边界，不能执行 tool 后再伪造失败。

### 4. 非目标

- `hello` 之外的本地 tool、动态 tool catalog、resource、prompt、notification 或业务 side effect；
- MCP-to-Provider Bridge、generation tool execution 或把 Chat/Responses tool call 交给本 endpoint 执行；
- browser Origin allowlist、远程公网部署或绕过 loopback/Bearer 边界；
- 将 stateless metadata 规则强加给 legacy session 请求，或移除当前 legacy lifecycle。

## 验收与非目标

下列 ID 是网关 API 的稳定行为约束。实施证据由[实施现状](../implementation-status/README.md)单独记录。

### 1. 功能验收要求

| ID | 应被保护的用户可观察行为 |
|---|---|
| API-01 | 有效静态 token 可访问标准/扩展 Models 与业务 endpoint；认证失败、未知 Public Model、不支持 feature 与非法请求在 egress 前安全失败。 |
| API-02 | 标准/扩展 Models 接口满足[模型能力契约](model-capability.md)的身份、逐字段一致性与部署信息隔离。 |
| API-03 | Native Chat/Responses 中已知且被接口接受的字段，除统一 instructions/store envelope、固定 reasoning 归一化、受信 model/auth 改写、Provider wire mapping 与闭合普通参数忽略规则外保持 wire 语义；未知请求顶层字段拒绝，上游响应的未知合法字段/event 不丢失。 |
| API-04 | SSE 分片、terminal、EOF、上游 error 与下游 cancel 不产生伪成功、重复 terminal 或跨 Target 拼接。 |
| API-05 | Chat/Responses 普通 function tool 的 call/result identity 与 fragmented arguments 在声明路径中保持；generation 链路不执行 tool。 |
| API-06 | Codex Native profile 只在受限 allowlist 内保留 turn-state 扩展；Bridge、Route change 或 fallback 不复用该状态。 |
| API-07 | Codex、OpenAI SDK 或 Hermes 等客户端专属承诺必须限定 endpoint、feature、transport 与版本，不得把专属 profile 扩大为通用兼容契约。 |
| API-08 | 客户端只选择 Public Model 与下游协议；固定契约不支持时拒绝，普通忽略参数只按选中 API 删除，其他请求保持固定 Route 顺序。 |
| API-09 | 无状态请求避开短时 cooldown 的 quota/fault scope；target-bound state 不因健康状态切换 issuing Target。 |
| API-10 | reasoning input 只接受 canonical vocabulary 与 Public Model `accepted_levels`；`strict` 保持精确值，`clamp_positive_floor` 只处理正向 effort，`none` 不参与转换；非法值在 egress 前失败。 |
| API-11 | 无状态 Responses 是默认契约：`store` 省略或 false 规范化为 Native egress 的显式 false，其他显式值拒绝；`background:false`/省略与 `previous_response_id:null`/省略可用，`background:true` 与非 null `previous_response_id` 在当前固定接口中 zero-egress 拒绝。 |
| API-12 | Embeddings、图片、文件与音频满足[扩展共同规则](extended-capabilities.md)及各功能页的 wire、能力、资源归属和限制。 |
| API-13 | token-bearing text/tool/reasoning SSE delta 只触发一次 TTFT/generation window；非流式成功 JSON 的 gateway-visible body timing 不伪造 upstream TTFT、generation duration 或 output speed；telemetry 不含正文或身份 secret。 |
| API-14 | 有效 token 可通过 `/mcp` 使用 `2026-07-28` stateless discovery 或 legacy initialize/session lifecycle 发现并调用唯一 `hello(name)`；两种 lifecycle 都执行相同认证、Origin 与无 Provider egress 边界，非法 metadata/session/tool/method 在执行前失败。 |
| API-15 | `include: []` 作为 no-op 在 candidate 展开前移除；非空 `include` 按 public accepted set 逐值预检，未知或未获批准的值 zero-egress；`reasoning.encrypted_content` 是唯一可按 candidate 原生转发或在 Native/Bridge planning 中删除的 include hint，任何残留 active `include` 到达 Bridge 必须失败；`prompt_cache_key` 作为 accepted best-effort hint 按 candidate 原样转发或删除且不承诺缓存效果。 |
| API-16 | Chat `stream:true` 下空 `stream_options` 与 `include_usage:false` 作为 no-op 移除；`include_usage:true` 只有固定 interface 完整保证时接受，Native 原样保留，Chat-to-Responses Bridge 只从合法 terminal usage 生成标准 usage-only 尾块。 |
| API-17 | 通用 Generation 只解析一次客户端 instructions 并在缺失时使用项目默认值；Native/Bridge/candidate/retry/probe 编码一致，首条合格 Chat 指令只提升删除一次，专用 task 不注入。 |
| API-18 | Responses `reasoning.summary` 接受 `"auto"` 与兼容 `false`：Native 精确保留，Responses-to-Chat 消费且只返回真实 Chat reasoning content，不伪造 summary；非法值与 `none+auto` 在 egress 前失败。 |
| API-19 | nullable prompt-cache 字段与无可执行 function tool 的 `parallel_tool_calls` 在 planning 前按 typed inactive 语义删除；active retention 保持 unimplemented；active parallel true 只由 toggleable contract 接受，active false 可精确转发或由显式 serial-only candidate 安全删除，所有非法 shape 与未证明控制均 zero-egress 失败。 |

### 2. 非目标

- GUI、Web 控制台或客户端安装/注册/配置管理；
- Realtime、Responses WebSocket、Files、Images、Videos、Conversations、response resource 或管理 API；
- response storage、background job、查询、删除、翻译、跨 Provider/Target state migration 或 continuation ledger；
- 让 Chat/Responses、任意 tool 或 Provider 私有扩展自动无损互转；
- 代表下游 Agent 执行 function tool、shell、computer 或网页操作；
- 在 MCP endpoint 执行 `hello` 之外的 tool、桥接 Provider、产生外部 side effect 或提供 browser Origin allowlist；
- 用 API token 建立多用户权限、配额、账单或审计系统。
