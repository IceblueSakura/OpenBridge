# tools、continuation 与扩展

## 状态

本文是[网关 API 域](README.md)的工具与状态模块：定义 function tools、统一 generation instructions、无状态核心、
状态亲和、普通参数上游兼容与 Responses 输出投影。其他模块见[网关 API 域](README.md)导航。

## 1. function tools

对于已声明支持的普通 `type: "function"` tool：

- 需要保持请求 schema、并行调用顺序、`call_id` / `tool_call_id`、arguments 分片和 tool result 的关联；
- Responses `input` 中标准 message 可以显式携带 `type: "message"`，也可以使用只包含 `role` 与 `content` 的 shorthand；
  Responses→Chat Bridge 必须对两种写法采用同一 message 转换，缺失 `type` 且含额外字段的模糊对象仍须拒绝；
- arguments 在完成前是未可信的字符串，网关不得执行或授权模型返回的工具调用；
- tool call/result、`item_id`、stream output index 与 request id 是不同身份，不能相互替代。

## 2. 统一 generation instructions 与无状态核心

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

## 3. 状态亲和与私有扩展

- `previous_response_id`、Provider resource、tool continuation、opaque reasoning 与 issuing call 都是可能绑定 Upstream
  Target/Upstream API 的状态。不能安全证明等价时，拒绝、保持同一 issuing target/upstream API，或要求完整可转换历史；不得跨候选猜测或
  replay。
- Codex 所需的 `x-codex-turn-state` 及 `response.metadata` 属于受限私有扩展：只在显式启用的 Codex Native Responses
  profile 中透明保留，不能进入 Bridge IR、用户 transcript、普通日志或跨 target fallback。
- MCP、custom tool、hosted tool、reasoning、annotation、image generation 等不是普通 text 的同义词。所选 Public Model
  固定接口未声明支持时必须在上游调用前拒绝，不得静默丢弃。

## 4. 普通生成参数的上游兼容

Canonical Model 已声明的普通 Chat/Responses 生成参数可以由下游提交，并继续出现在对应 interface 的
`supported_parameters`。若某个具体 Upstream API 已由官方文档或真实请求确认不接受其中一个参数，代码注册表可以通过闭合、类型化规则
将该字段标记为"下游接受、上游忽略"。选中该 API 后，OpenBridge 必须在 candidate 绑定完成后、进入第一个无法表达该字段的
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
忽略字段表示"OpenBridge 接受请求"，不保证每个候选上游都会应用该提示；这一例外不得扩展为任意字符串、用户可配置或请求可选择的过滤器，
也不得提供任意 `extra_body` 绕过类型化目录和固定能力预检。

## 5. Responses 输出投影与缓存键转发

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

Responses 标准 event 见[Responses typed SSE 调研](../../references/openai/responses/streaming.md)；Codex 私有扩展仍由对应 Codex
项目调研维护。

## 关联文档

- [网关 API 域导航](README.md)
- [Native Path 与流式语义](native-path-and-streaming.md)
- [请求、Public Model 与安全边界](request-and-security-boundary.md)
- [路由与 Provider 韧性](../routing-resilience/provider-resilience.md)
- [当前实现总览](../../implementation-status/current-implementation.md)
