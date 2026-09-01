## 1. Native Path 基线

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

## 2. 流式语义

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

## 3. 遥测计时边界

运行期 TTFT 与输出速度必须以实际 token-bearing SSE delta 为边界：除 text 和 function arguments 外，Native wire 中明确出现的
reasoning text delta 也属于生成输出。TTFT、首字节和首输出均只记录第一次命中；后续 chunk/delta 不得重复执行同一聚合热路径。
输出速度的时间窗口从首个上述生成 delta 到原始 upstream body 完成，使 reasoning token 不会进入分子却把其生成时间排除在分母。
该遥测识别不扩大 Public Model reasoning capability，也不授权 Bridge 转换未知 reasoning wire。
