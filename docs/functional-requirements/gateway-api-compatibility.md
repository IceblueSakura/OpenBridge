# 网关 API 与客户端兼容需求

## 状态

**当前目标。** 本文定义 OpenBridge 对下游客户端可见的 API、认证、原生 HTTP/SSE 语义和兼容边界；不规定内部模块、converter
形态或实现顺序。当前已经由代码和测试证明的范围以[当前实现总览](../implementation-status/current-implementation.md)链接的功能专题为准。

## 1. 用户结果

受信用户应能把本地 Agent 或 OpenAI-compatible SDK 指向一个稳定的 OpenAI-compatible base URL，使用私有用户表中分配的
Bearer API Key 与 Public Model 调用服务。主要调用路径不得要求客户端知道上游 Provider、真实模型、URL、凭证或候选切换细节。

初期的兼容目标按优先级为：

1. OpenAI SDK 的 Chat Completions 与 Responses HTTP JSON/SSE；
2. OpenAI-compatible Embeddings，以及 Chat/Responses 同协议 Native 多模态输入；
3. 独立 Python 脚本或 curl 的最小 HTTP/header/SSE 复现；
4. 只有在明确声明时，才验证 Codex、Hermes 等具体客户端的 profile、transport 与 tool-loop 行为。

“某个请求能被转发”不等于“某个 Agent 已完整兼容”。每项声明必须限定 endpoint、stream、tool、continuation、Provider 与实际验证版本。

## 2. 接口与认证

| 接口                                                             | 功能要求                                                                                                              | 不包含的语义                                                                                 |
|------------------------------------------------------------------|-----------------------------------------------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------|
| `GET /healthz`                                                   | 提供不访问上游凭证的最小本地存活信息；不得泄露 route、Upstream Target 或 secret。                                     | Provider 健康探测、控制面或客户端管理。                                                      |
| `GET /v1/models`、`GET /v1/models/{model}`                       | 按[模型能力契约](model-information-and-capability-contract.md)返回严格的 OpenAI 标准四字段 list/retrieve。            | 扩展能力、上游模型或部署信息。                                                               |
| `GET /openbridge/v1/models`、`GET /openbridge/v1/models/{model}` | 返回同一 Public Model 目录的模型事实和 Chat/Responses/Embeddings 固定能力契约。                                       | Provider/target/route、credential、健康、价格或动态发现。                                    |
| `POST /v1/chat/completions`                                      | 支持已声明能力范围内的 Chat JSON/SSE，并按[扩展需求](embedding-and-native-multimodal.md)提供 Native 多模态输入。      | 多模态 Bridge、audio output、专用媒体/资源 API 或 hosted tool 的默认兼容承诺。               |
| `POST /v1/responses`                                             | 支持已声明能力范围内的 Responses JSON/SSE，并按[扩展需求](embedding-and-native-multimodal.md)提供 Native 多模态输入。 | 多模态 Bridge、Responses WebSocket、资源 retrieve/cancel/store/background/conversation API。 |
| `POST /v1/embeddings`                                            | 支持独立 Embedding Public Model 的 OpenAI-compatible JSON 请求/响应。                                                 | streaming、向量转换/存储/检索，或无等价证明的跨模型 fallback。                               |

业务 endpoint 必须使用用户表分配的静态 Bearer API Key。用户表只在启动时读取，不提供在线 key issuance、scope、即时撤销、配额或
billing identity；变更需要重启。认证失败与未知/不支持 endpoint 必须在进入路由或上游调用前结束，且不泄露配置细节。

## 3. 请求、Public Model 与安全边界

### 3.1 Public Model 与 routes

- 下游只能提供已配置的 Public Model；它表示 OpenBridge 对下游提供的稳定服务契约，而不是某个上游模型名的透明别名。身份、生命周期、固定能力计算、Models
  API 和错误语义统一由[Public Model 与模型能力契约](model-information-and-capability-contract.md)定义。
- 请求能力只在所选 Public Model 边界预检一次，不参与选模、Route 候选资格、顺序或 fallback。预检通过后，Route 仍按配置顺序固定
  Upstream Target、Upstream API、下游 operation 和执行模式；generation `Native` 要求协议相同，`Bridged` 要求协议相反且通过完整
  `BridgePlan` preflight，Embeddings 只允许同 operation Native。
- 服务对上游只使用选中 route 的真实模型名、协议、endpoint 与 credential；下游不能通过 body、query 或 header 指定上游
  URL、模型、credential、provider family、route、转换脚本或 header 转换规则。Provider 的受信代码 hook 可以按编译期规则增添、替换、转换或删除普通
  header，但认证、cookie、Host 与 proxy header 始终隔离。
- 请求开始后，Public Model、RoutePlan、credential pool binding 与注册表版本保持固定；无状态 attempt 可按策略选择 pool
  member。

### 3.2 输入保护

- 仅接受端点契约允许的 content type、JSON body 和受配置约束的大小；无法安全解析的请求在 egress 前返回稳定错误。
- 请求分类必须先识别 operation，再按 operation 解析 `stream`、input form、function/custom/hosted
  tool、并行工具、结构化输出、multimodal、reasoning、`previous_response_id`、background/store 与相应限制等会影响固定契约或状态边界的特征。
- 未知 feature 不能因“目标 Provider 也许支持”而默认放行到 bridge；Native Path 可保留同协议的未知合法字段，前提是它们不绕过固定契约、安全或
  state-affinity 决策。
- 服务为每个请求生成或传播安全的 request id，用于响应和受控诊断；该 id 不是 client identity、tool identity 或聚合指标
  label。

## 4. Native Path 与流式语义

当下游与上游协议一致且请求已通过 Public Model 固定契约预检时，Native Path 是兼容性基线：它只做受信路由、模型、认证和 显式
reasoning level wire 映射，保留其他 JSON、HTTP status、必要 allowlist header 与未知合法字段，不经过 通用 IR 重渲染。level
映射必须属于选定 Upstream API 的代码注册规则，映射源必须已由 canonical Model 声明， 目标必须是安全 wire
值；不得由业务请求提供映射或用映射扩大 Public Model 支持的下游 level 集合。 canonical reasoning level vocabulary 为
`none`、`minimal`、`low`、`medium`、`high`、`xhigh`、`max`； 每个 Model 仍须显式声明实际支持的子集。`none` 是调用方显式要求禁用
reasoning，不等同于缺少 reasoning 字段。

Embeddings Native Path 使用独立严格 JSON request union 和有界 JSON response validator；不保留未知字段，不进入 generation
SSE/Bridge，也不在网关转换 vector encoding 或 dimensions。客户端必须以所选
`interfaces.embeddings` 的 forms、domain、parameters 与有效 limits 为准。

显式 `Bridged` Route 必须只转换两协议共同可表达且已由 Upstream API capability 确认可读、方向兼容的 reasoning
channel、text、function schema、tool call/result identity、非流式 JSON 和流式 SSE lifecycle；
`Unknown`、`Unsupported`、`Opaque` reasoning 输出在需要跨协议传递时必须拒绝。未知顶层字段、opaque continuation、
hosted/custom tool、image、structured output、background/store 和 Provider 私有扩展必须在 egress 前拒绝。 Bridge
不能因字段名相似、Provider 名称或 capability 并集猜测转换；没有完整 Native/Bridged Route 时返回稳定能力错误。

流式请求必须满足：

- 原样保持协议的 SSE framing、event/data 负载与输出顺序；不得注入 OpenBridge 自定义 SSE event。
- Chat 以其自身终态（包括 `[DONE]`）处理；Responses 区分 item/content lifecycle 与 `response.completed`、
  `response.incomplete`、`response.failed`、`response.cancelled` 或顶层 `error` 等 response terminal。
- `output_item.done`、tool input delta、metadata/header 到达或任意首字节都不等于请求成功。已写出首个业务 body byte 后，不得
  retry、fallback 或将其他 Upstream Target 的内容拼入当前 stream。
- 下游取消、连接中断、deadline 和错误终态应停止相应上游工作；合法但无 terminal 的 EOF 不得伪造成 completed。
- response headers 和 SSE bytes 的处理必须受大小、UTF-8、event 数量/长度与慢消费者资源上限保护。

运行期 TTFT 与输出速度必须以实际 token-bearing SSE delta 为边界：除 text 和 function arguments 外，Native wire 中明确出现的
reasoning text delta 也属于生成输出。TTFT、首字节和首输出均只记录第一次命中；后续 chunk/delta 不得重复执行同一聚合热路径。
输出速度的时间窗口从首个上述生成 delta 到原始 upstream body 完成，使 reasoning token 不会进入分子却把其生成时间排除在分母。
该遥测识别不扩大 Public Model reasoning capability，也不授权 Bridge 转换未知 reasoning wire。

## 5. tools、continuation 与扩展

### 5.1 function tools

对于已声明支持的普通 `type: "function"` tool：

- 需要保持请求 schema、并行调用顺序、`call_id` / `tool_call_id`、arguments 分片和 tool result 的关联；
- arguments 在完成前是未可信的字符串，网关不得执行或授权模型返回的工具调用；
- tool call/result、`item_id`、stream output index 与 request id 是不同身份，不能相互替代。

### 5.2 无状态核心与有状态 Native pass-through

OpenBridge 的核心实现重点是无状态服务；无状态请求是默认使用方式、主要客户端兼容面和当前验收基线。客户端必须在每次请求中
携带所需完整历史，`store` 应省略或为 `false`，`previous_response_id` 应省略或为 `null`，`background` 应省略或为 `false`。
该路径可以在完整能力约束下使用 Native Route、有限 retry/fallback，以及仅转换显式共同语义的 Bridge。

`previous_response_id`、`background` 与 `store: true` 是次要目标。当前实现只有能力 gate、状态亲和与唯一签发者校验等基础约束，
尚未形成完整的 response state storage、retrieve/cancel、conversation lifecycle 或 continuation ledger；它们不能作为通用会话、
后台任务或跨 Provider 状态迁移能力使用。正常接入、示例和验收不得以这些字段为前提。

有状态 Responses 当前只作为能力受限的 Native pass-through：

- `store: true` 只能进入明确声明该能力的 Native Responses Upstream API；不得通过 Bridge 实现或静默改写为
  `false`；
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

Responses 标准 event 与 Codex 私有扩展的细节见[Responses 协议参考](../references/openai/responses-protocol.md)。

## 6. 错误与客户端可见结果

| 时机                                        | 必需行为                                                                                                                                              |
|---------------------------------------------|-------------------------------------------------------------------------------------------------------------------------------------------------------|
| ingress、Public Model、能力、认证或配置拒绝 | 上游调用前返回安全、稳定的 OpenAI-compatible JSON error；不暴露 URL、credential、候选列表或内部栈。                                                   |
| 所选 Public Model 的固定接口契约不支持请求  | 在 egress 前返回稳定 `unsupported_model_capability`；不得改选模型、筛选 Route 或静默丢失字段。                                                        |
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
| API-03 | Native Chat/Responses JSON 与 SSE 除受信模型/认证改写外保持 wire 语义；未知合法同协议字段/event 不因网关丢失。                                                                     |
| API-04 | SSE 分片、终态、EOF、上游 error 和下游 cancel 不会产生伪成功、重复 terminal 或跨 Upstream Target 拼接。                                                                            |
| API-05 | 普通 function tool 的 call/result identity 与 fragmented arguments 在已声明路径中保持；网关不执行工具。                                                                            |
| API-06 | Codex Native profile 能在受限 allowlist 下保留其已验证的 turn-state 扩展；bridge、route change 或 fallback 不会误复用该状态。                                                      |
| API-07 | 对 Codex、OpenAI SDK 或 Hermes 的兼容声明均有相应 endpoint/feature 的可重复证据，并写入实施现状而非仅引用设计。                                                                    |
| API-08 | 客户端只选择 Public Model 与下游协议；固定能力契约不支持时统一拒绝，支持时保持配置 Route 顺序，不按请求能力筛选或重排候选。                                                        |
| API-09 | 无状态请求避开短时 cooldown 的 quota/fault scope；target-bound continuation 不因健康状态切换 issuing target。                                                                      |
| API-10 | Native reasoning level 只接受 canonical vocabulary 中由 Model 显式声明的值，并按选定 Upstream API 的已校验规则改写；未知或未声明的下游 level、歧义源或非法目标在 egress 前失败。   |
| API-11 | 无状态 Responses 是核心兼容面、默认使用方式和当前验收基线；`store: true`、非空 `previous_response_id` 与 `background: true` 仅作为次要且不完整的 Native pass-through 目标，不进入 Bridge、跨 Target fallback 或状态迁移。 |
| API-12 | Embeddings 与 Native 多模态满足[扩展需求](embedding-and-native-multimodal.md)的 wire、能力、资源归属、限制和证据边界。                                                             |
| API-13 | token-bearing text/tool/reasoning SSE delta 只触发一次 TTFT/生成窗口，非流式 Chat/Responses 成功 JSON 只在首个非空下游 body chunk 记录一次可直接观测的 gateway TTFT，不得据此伪造 upstream TTFT、生成时长或输出速度；OTLP metrics 不含请求正文、响应正文、Authorization、credential、用户或 request ID。 |
| OBS-01 | OTLP exporter 默认禁用；只有合法的 startup-only OTLP/HTTP 配置能启用相应 signal，collector host 可由配置所有者选择，非法配置在 listener 和 exporter egress 前失败，业务请求无法覆盖。 |
| OBS-02 | 一个已认证业务请求产生一个脱敏 request root span，每个实际 Provider attempt 产生一个有序 child span；terminal、retry、fallback、失败与取消不重复也不改变实际因果关系。       |
| OBS-03 | OTLP metrics 使用 SDK 原生 counter/histogram 和有界维度；单 attempt output speed 只由明确 output usage 与 generation duration 计算，分位数、平均值、错误率、缓存 token 比例与 Provider + Public Model 排名由外部系统计算，未知值不补零。 |
| OBS-04 | OTLP logs 只导出安全、限频且可通过 trace/span id 关联的运行诊断；不记录逐 chunk/delta，也不复制完整 request/attempt terminal 形成冗余高频日志。                              |
| OBS-05 | export 使用有界异步队列和有界关闭；collector 故障、超时或背压不阻塞请求、不改变 HTTP/SSE/Provider 行为，只允许丢弃 telemetry 并产生限频本地诊断。                          |
| OBS-06 | 所有 signals 都不包含 credential、Authorization、用户身份、业务正文、tool/reasoning 内容、原始错误正文、query 或真实 endpoint URL；metric attributes 不含高基数身份。       |
| OBS-07 | OTLP metrics 覆盖 request/attempt、韧性、timing、usage 与 cache 事实后，旧 metrics HTTP endpoint 和自定义 snapshot 聚合保持删除，不为未发布原型保留兼容垫片。 |

## 9. 非目标

- GUI、Web 控制台、客户端安装/注册/配置管理；
- Realtime、Responses WebSocket、Files、Images、Videos、Conversations、管理 API 或“实现全部 OpenAI API”；
- 保存、查询、删除、翻译或跨 Provider/Target 迁移 response 状态，以及未有真实需求前实现 continuation ledger；
- 让 Chat ↔ Responses、任何 tool 或 Provider 私有扩展自动无损互转；
- 代表下游 Agent 执行任意 function tool、shell、computer 或网页操作；
- 用 API token 建立多用户权限、配额、账单或审计系统。

## 关联文档

- [产品范围](product-scope.md)
- [Public Model 与模型能力契约](model-information-and-capability-contract.md)
- [配置与凭证](configuration-and-credentials.md)
- [路由与 Provider 韧性](provider-resilience.md)
- [交付与证据要求](delivery-and-evidence.md)
- [当前实现总览](../implementation-status/current-implementation.md)
