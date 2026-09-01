本文拥有普通 generation 参数的受控上游忽略规则，以及 Responses `include` 和 prompt-cache 请求字段。

## 1. 普通 generation 参数

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

## 2. Responses `include`

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

## 3. Prompt-cache 与 parallel-tool 控制

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
