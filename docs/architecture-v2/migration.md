# Responses-first 实施基线与差距

目标结构由 [semantic-ir](semantic-ir.md)拥有，标准事实与上游固定点见[主题参考](../references/README.md)。本页描述当前代码相对目标的差距，不把设计或删除旧代码当作实现完成。

## 当前状态

当前 workspace 是 v2 `semantic`、`protocol`、`lowering` 和纯 SSE 库，旧 runtime 已[归档](../archive.md)。无生产 Provider/registry、credential、服务入口、state resource 或 WebSocket execution。内部兼容性不要求保留。

当前有序文本/function/custom/reasoning、部分控制、annotations/logprobs、usage、Static/Event 与 HTTP/SSE 测试已有 owner，但仍是受限 stateless text profile。固定 SDK gates 的场景与执行入口见[开发指南](../development.md#固定-openai-sdk-loopback)，依赖由 `tests/sdk/` 锁定；这些局部验收不覆盖下列全部标准分支。

单候选 Chat 的完整 JSON/SSE 字节链路复用同一 IR、strict JSON 与 framer，具体准入见 [Chat profile](chat-text-profile.md)。该 profile 不包含全部 Chat 可选字段；SDK 派生 view 仅准入 message 级 `parsed` 回放（function `parsed_arguments` 投影除外），也不将 Responses-only 能力降格为不可表达的 IR。

## 标准目标与实现映射

| 域 | 当前实现 | 相对目标的缺口 |
|---|---|---|
| Instructions/messages | `GenerationSettings`、ordered Item、text/refusal | assistant phase；非完成 instruction lifecycle 的表示；其余标准 input/output 分支准入 |
| Controls/Schema | typed 外层控制、有序 schema、结构/strict/default 准入、本地引用和独立预算 | regex/format 语法与模型级限制；当前 schema profile 之外的词汇；其余控制的 null/default 规则 |
| Function/custom | 定义、choice、calls、文本/文本数组 outputs、事件 | namespace、caller/programmatic/async/deferred、标准工具多模态结果及 Chat function `parsed_arguments` 回放 |
| Reasoning | effort/context/mode、readable summary/text、origin-bound replay | 标准 configuration update；当前 `summary:false` 与标准 profile 的区分；更完整 snapshot/event 准入 |
| Full response | identity、settings echo、usage、status/details | 其余标准 item 分支必填性审查；request hints 与 effective response context 的分支合同 |
| Events/SSE | reducer、framing、byte/event/semantic/padding budgets | 完整标准事件矩阵；本地 cancelled event 的 profile 分类 |
| 标准媒体 | 仅基础 Resource 类型；codec 拒绝 | image/file source、detail、filename、cache boundary、media tool result 的双向映射 |
| 标准 hosted/state | 当前明确拒绝或只允许 inactive | 标准 tools/items/approval/progress；previous/conversation/store/background、prompt、compaction/reference 的表示与独立执行 |
| Request context | 当前 `ExecutionHints` 仅部分 typed 标准字段 | state unit stubs、cache prewarm、新标准分支；不应把所有 context 称为“无需 IR 的执行杂项” |
| 扩展 | fidelity 保存少量 identity/form/replay | 任务/part/resource/context 的统一准入合同；Codex session/cache/thread/turn；namespace/schema/scope/visibility/requirements |
| WS / resource operations | 没有实现 | lane/multiplex/steering、connection state 与单 response reducer 的边界；retrieve/cancel 等资源操作 |

本表不是全部 API schema 的逐字段完成矩阵。外部验收方法由[验收基线](../references/conformance-baseline.md)拥有。

## 当前已知闭合缺口

以下缺口仍存在；准入规则与已实现拒绝边界由 [text profile](responses-text-profile.md) 和独立测试维护。

1. Schema 的当前有限准入由 [schema profile](schema-profile.md)定义；不执行 regex/format，不验证模型输出 adherence，也不保证所有模型接受。未知方言、动态/远程引用及未准入词汇明确拒绝；不能把局部验证当成完整 JSON Schema 引擎。
2. SDK function `parsed_arguments` 的 Chat 投影未准入：`ParsedFunction` 无 `__api_exclude__`，SDK 序列化会把派生值带进回放的 `tool_calls`；Responses `function_call` 条目已有独立一致性规则，不能自动转移。parsed text 派生 view 的双协议回放准入见 [derived replay rules](responses-text-profile.md#derived-replay-views)。
3. 标准 phase/configuration update、更多 media/tool/event/state 分支没有实现；`summary:false`、cancelled event 等现有兼容分支需 profile 分类。

上述缺口依据当前源码与固定标准对照，不代表真实 Provider 的能力或实测差异。

## Phase gates

### A. 基线与准入

采用 Responses-first + scoped extensions，按 request/item/part/tool/response/event/context 列出标准与扩展边界。确定 required/presence、origin、生命周期、保真与拒绝，消除旧 ADR 和当前源码冲突。设计文档完成不能将字段标为已实现。

### B. 已支持子集的正确性

保持 IR 权威、完整 message 必填性和已实现的显式拒绝规则；继续补齐派生 SDK view，并按实际目标完善 Schema profile、审查其余字段/事件分支。每项先独立失败用例，再同步 semantic、requirements、lowering、codec。保留当前 Responses text 验收，不从零重写正常工作机制。

### C. 标准表达力扩展

优先补 phase/configuration 等影响现有续轮的标准语义，再按明确范围推进媒体、标准 tools 和 state/resource references。每个域同时考虑 request、response、event、变换与失败闭合。当前 runtime 缺失不应导致 IR 永久缩成 stateless text，但表示支持不自动开启执行。

### D. 特殊能力扩展

先定 downstream extension wire/版本与实际来源 profile，再实现 Codex context 或明确特殊媒体。扩展 codec 需来源/owner/生命周期测试，不引入万能 extra JSON、动态插件框架或旧 Gateway tools 原型。

### E. 执行接线

在对应语义域可验收后，独立设计 trusted topology、credentials、state ownership、resource I/O、WS 和 ingress。不要求所有未来任务完成才可接线，也不以某个旧目录存在作为迁移依据；每个执行切片单独明确授权、影响与验证层。

## 固定不变量

- 最终标准语义和 typed extension 是编码权威；source/fidelity 不恢复已删除值。
- 每个候选从同一不可变内部表示投影；Chat 可表示性不反向缩减 Responses IR。
- 标准字段不能放进 extension 绕过类型验证，extension 不能覆盖标准或提供 auth/route/endpoint。
- 离线 SDK/codec 成功不证明 live Provider、Agent、媒体质量、生产或长期稳定性。

实施范围由 [current-focus](../implementation-plans/current-focus.md)记录，不由本路线图自动授予。未完成项不能因其他切片通过测试而视为已修复。
