本文拥有 Chat/Responses 的统一 instructions、无状态默认与 Responses state 字段拒绝契约。

## 1. 统一 instructions

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

## 2. 无状态默认

客户端默认携带完整历史，并使用：

- `store` 省略或为 `false`；
- `previous_response_id` 省略或为 `null`；
- `background` 省略或为 `false`。

该路径可以在固定能力契约内使用 Native Route、有限 retry/fallback，以及只转换共同语义的 Bridge。

当前 allow/deny 行为固定为：

- `store:true`、`store:null` 及其他非布尔显式值稳定失败；每个 Native Responses candidate 显式编码
  `store:false`，Responses-to-Chat Bridge 消费该事实而不向 Chat wire 添加字段。
- `background:false` 或省略表示同步请求；Public Model interface 永久不公开 background capability，
  `background:true` 在 Provider egress 前拒绝。
- `previous_response_id:null` 或省略表示无 continuation；Public Model interface 永久不公开 response-ID
  continuation，任何非 `null` 值都在 Provider egress 前拒绝。
- OpenBridge 不提供 response store/retrieve/cancel/delete、background job、conversation lifecycle 或
  continuation ledger。客户端不得把这些字段当作通用会话或后台任务能力。

## 3. 状态拒绝的永久性

上游 Provider 有状态 API 是永久非目标（见[产品范围](../product-scope.md)支持层级一节）：`store`、
`background`、`previous_response_id`、response 状态存储与 continuation 不存在"未来扩大"路径。

为此保留一条防御性启动约束：如果注册代码以可贡献 `previous_response_id` 的 executable state
（`TargetBoundContinuation`）声明某个 Responses API，registry 必须像对待其他非法注册一样在启动时拒绝
不完整或不安全的启用条件，而不是在请求期猜测账号/Target 亲和。该变体不构成任何公开能力，也不为任何
下游参数放行。
