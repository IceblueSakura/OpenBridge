# Generation 实施边界与缺口

本页只记录当前设计尚未闭合的边界，不维护完成度矩阵或测试结果。[语义设计](../architecture-v2/semantic-ir.md)定义架构目标；协议合同分别查[来源索引](../references/README.md)，现有 OpenAI/Codex 基线见[固定来源](../references/upstream-sync.md)；实际准入查 [adapters](../../src/adapter/mod.rs)、[protocol codecs](../../src/protocol/openai/mod.rs)、[lowering](../../src/lowering/generation.rs)及独立测试。推进顺序只由 [next-goal](../implementation-plans/next-goal.md)维护。

Generation 的目标是 Agent-first、协议中立；当前类型与 codec 仍以 Responses/Chat 切片为基础，不承诺任意双向转换。文本输出及 user URL/inline 图片输入的具体合同见 [Responses](../architecture-v2/responses-text-profile.md) / [Chat](../architecture-v2/chat-text-profile.md) profiles；库类型、HTTP 接线、实例启用和上游接受必须分别核查。

## 语义与表示缺口

- **交互与依赖设计落地**：跨 response 的逻辑 turn/continuation、跨协议 typed group 与非 reasoning attachment 的 replay 依赖尚未形成完整共享合同及主链；[消息组视图](../../src/semantic/task/generation/group.rs)仅覆盖已声明的 assistant/function-call 归属，不表达共同 replay 或前缀依赖；现有 response outcome 与[只读 pending-call continuation view](../../src/semantic/task/generation/continuation.rs)覆盖完成响应及最终 history 的缺失结果关联，但不表达完整 turn 进度、组依赖或其他协议的暂停续轮；Responses reasoning 可由受信调用方显式绑定[进程内 group/prefix/settings 证明](../../src/semantic/task/generation/dependency.rs)，但默认 codec 不推断这些依赖，其他 attachment 与客户端回传 carrier 仍未闭合。新的设计基线不自动扩大当前 profile 准入。
- **续轮就绪与跨 response 关联**：结果按 kind/call ID 关联、部分/全部结果到达后的待结果引用不等于完整 continuation 合同。[显式本地关联](../../src/semantic/task/generation/turn.rs)仅覆盖一个 completed response 的结果齐备检查及调用方声明的直接后继关系；仍缺上游报告的 turn 进度、完整关联链准入、group/opaque/设置依赖的整体满足判据，以及部分结果 history 的执行准入/调度合同；`Unreported` 不能当作 turn 已完成或可发送下一请求。纯 history 视图不扩大 wire 准入、不恢复终态 response、不提供自动 Agent loop。现有 profile 边界见 [continuation 合同](../architecture-v2/responses-text-profile.md#response-outcome-and-continuation)。
- **标准分组 carrier 缺失**：Responses 标准 wire 没有 `ToolCall.message` 的承载字段；当前也没有获准的私有 carrier 或有损转换。带显式归属的 Chat tool-only/text+tool history 与输出不能无损投影为 Responses，request/static lowering 与首次 attached-call 事件均明确拒绝。独立 message/call 仍可表示，但不由邻接恢复归属；独立 calls 经具名 Chat call-run 投影后形成的显式归属也不能无损回投 Responses。缺失的是客户端交付→保存→回传的分组保真合同，不是 call ID 或 message item；进程内依赖证明不补足 wire carrier，更广 group/prefix 的协议依赖选择及 replay 交付仍待独立定稿。精确行为见 [Responses 分组边界](../architecture-v2/responses-text-profile.md#message-owners-and-cross-protocol-grouping)。
- **能力域演进**：资源关联与来源坐标引用、typed 前缀缓存意图、多协议 usage 口径与合法派生仍需独立定稿和实现。[工具结果值](../../src/semantic/task/generation/tool_result.rs)已提供库级结构化 JSON、错误及选定 text/image parts，但这些新语义尚无 Chat/Responses carrier，lowering 与低层 request 编码均拒绝；完整媒体、结果 Schema adherence 和资源生命周期仍未闭合。已有 Resource、ToolOutput、Usage 或 cache/fidelity 字段不等于这些设计已完整表达。
- **Chat 请求覆盖**：部分文本控制与 history 字段仍未准入。以 [Chat codec](../../src/protocol/openai/chat.rs) 的字段准入和 role shells 对照固定 SDK，不维护第二份字段清单。按具体消费需求决定 owner、映射和独立反例，不以追平全部字段为默认目标。
- **目标无损表示**：并非每种 reasoning、custom/program、phase、概率、reported context 或多 part grouping 都有对端位置。存在对应语义时由 lowering 拒绝，不丢字段或补默认值强行兼容；某个 shell 能表达 Schema 也不代表 strict 缺省语义相同。
- **有状态 API**：活动 continuation、conversation、存储/background、资源操作、compaction 和 WebSocket 会话尚无完整执行合同。接受 inactive 形式或 queued 事件不代表提供状态服务。
- **其他语义域**：file/resource ID 生命周期、工具媒体结果的 wire 交付、媒体输出/事件、音视频、hosted/dynamic tools 与独立任务未形成完整主链。选定图片输入不能代表完整多模态支持；标准缺口不是 generic extension。

## 扩展与执行缺口

- [CustomSections / CodexHeaders](../../src/protocol/extensions.rs) 的低层 carrier 不等于 gateway 支持。[Adapter request](../../src/adapter/request.rs) 限制 body sections；Codex headers 尚未接入 HTTP 主链。响应自定义段、typed observation headers、body/header 一致性、版本与 turn 生命周期需要独立定稿。
- configuration/program/cache 控制的表示不授权运行时应用设置、执行 program、管理 turn 或扩展 prewarm 执行语义。
- 上游 OAuth 登录、token refresh 与账户绑定生命周期尚未接入当前 Gateway；Bearer header 编码不等于 OAuth 登录。来源入口见 [OAuth 登录资料](../references/oauth-login.md)，推进方向由 [next-goal](../implementation-plans/next-goal.md) 维护。
- 固定 Route fallback 不提供凭据池、动态 registry、同候选自动 retry 或 session affinity，也不授权跨账户认证恢复。跨候选 opaque replay 不能由 canonical model 相同推定安全。
- 内部 replay scope 绑定不是 client token 的来源证明；源头真实性仍由 issuer 验证。生产观测、负载和长期资源保障不能由最小 loopback 网关推定。

## 验收缺口

固定标准 union 尚无完整逐分支 required/null/跨 kind 和组合审计。独立回归入口在 [semantic tests](../../tests/semantic.rs)、[transport tests](../../tests/transport.rs)、[gateway tests](../../tests/gateway.rs)；它们不是完整覆盖或最近执行通过的声明。

[固定 SDK gates](../../tests/sdk_loopback.rs)是单独的显式检查，不随默认 Rust tests 执行。一般 SDK/Agent 消费、真实 Provider/TLS/网络、缓存命中与收益、模型输出质量、负载和生产稳定性都需要独立证据。执行方法见[开发指南](../development.md)，结果不写入本页。

新发现的问题应区分错误接受、未准入、不可表示、未接线与缺少验收。获准修复先写入[当前焦点](../implementation-plans/current-focus.md)，再以最小独立回归保护边界；修复后移除对应缺口，不追加完成日志。
