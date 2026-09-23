# Responses-first 实施基线与差距

目标结构由 [semantic-ir](semantic-ir.md)拥有，标准事实与上游固定点见[主题参考](../references/README.md)。本页描述当前代码相对目标的差距，不把设计或删除旧代码当作实现完成。

## 当前状态

当前 workspace 是 v2 `semantic`、`protocol`、`lowering` 和纯 SSE 库，旧 runtime 已[归档](../archive.md)。无生产 Provider/registry、credential、服务入口、state resource 或 WebSocket execution。内部兼容性不要求保留。

当前有序文本/function/custom/reasoning、部分控制、annotations/logprobs、usage、Static/Event 与 HTTP/SSE 测试已有 owner，但仍是受限 stateless text profile。SDK gate 及其 Python 依赖由 `tests/sdk/pyproject.toml` 和 `uv.lock` 固定，当前 `openai==3.19.0` 的 synthetic 两轮 JSON/SSE 通过；不表示下列标准分支或已知缺口已验收。

## 标准目标与实现映射

| 域 | 当前实现 | 相对目标的缺口 |
|---|---|---|
| Instructions/messages | `GenerationSettings`、ordered Item、text/refusal | assistant phase；instruction status 的合法性/归一化；标准 input/output 分支必填性 |
| Controls/Schema | typed 外层控制，schema 为 bounded Value | TextOptions presence 矛盾；strict 方言、嵌套/reference 预算与 schema key 顺序；null/default 逐字段规则 |
| Function/custom | 定义、choice、calls、文本/文本数组 outputs、事件 | namespace、caller/programmatic/async/deferred、标准工具多模态结果及 SDK parsed text 回放 |
| Reasoning | effort/context/mode、readable summary/text、origin-bound replay | 标准 configuration update；当前 `summary:false` 与标准 profile 的区分；更完整 snapshot/event 准入 |
| Full response | identity、settings echo、usage、status/details | 缺必填 message id/status 仍补默认；request hints 与 effective response context 的分支合同 |
| Events/SSE | reducer、framing、byte/event/semantic/padding budgets | annotation 缺失检查、严格 JSON 重复键；完整标准事件矩阵；本地 cancelled event 的 profile 分类 |
| 标准媒体 | 仅基础 Resource 类型；codec 拒绝 | image/file source、detail、filename、cache boundary、media tool result 的双向映射 |
| 标准 hosted/state | 当前明确拒绝或只允许 inactive | 标准 tools/items/approval/progress；previous/conversation/store/background、prompt、compaction/reference 的表示与独立执行 |
| Request context | 当前 `ExecutionHints` 仅部分 typed 标准字段 | state unit stubs、cache prewarm、新标准分支；不应把所有 context 称为“无需 IR 的执行杂项” |
| 扩展 | fidelity 保存少量 identity/form/replay | 任务/part/resource/context 的统一准入合同；Codex session/cache/thread/turn；namespace/schema/scope/visibility/requirements |
| WS / resource operations | 没有实现 | lane/multiplex/steering、connection state 与单 response reducer 的边界；retrieve/cancel 等资源操作 |

本表不是全部 API schema 的逐字段完成矩阵。外部验收方法由[验收基线](../references/conformance-baseline.md)拥有。

## 当前已知闭合缺口

以 `5924f80` 的源码及本地 synthetic 最小复现为依据，不表示本轮已修复：

1. `TextOptions.presence=false` 与非空 format 仍通过 validation/lowering，requirements 报 structured output，encode 却省略 `text`。入口：`request.rs`、`output.rs`、`settings.rs`。
2. 完整 response 缺 message id/status 仍被接受，重新编码补 ID/completed。入口：`envelope.rs::validate_item_snapshot`、`responses.rs`。
3. annotation-added 缺 payload 被忽略，instruction message 非法 status 被吞掉。入口：`events/decode.rs::annotation_added`、`responses.rs::decode_items`。
4. SSE `serde_json::from_str::<Value>` 接受重复 key；静态测试 handler 同样没有严格 bytes parser。重复键检测必须早于 Value。
5. schema 只检查 object/bytes，非法 nested keywords 可通过。入口：`validate.rs::schema`。另据本次规范对照，Structured Outputs 按 schema key 顺序生成，当前 `serde_json` 未启用 preserve-order，需要专门验证顺序保持与 strict 方言。
6. SDK parsed text 派生 `parsed` 回放被 `text.rs` 拒绝；已有 `parsed_arguments` 规则不覆盖它。
7. 标准 phase/configuration update、更多 media/tool/event/state 分支没有实现；`summary:false`、cancelled event 等现有兼容分支需 profile 分类。

前六项分别执行过最小反例；第 5 项中的属性顺序以及第 7 项是本轮源码/规范对照发现，尚未补项目测试。相关事实不称为 Provider 实测差异。只改文档不改变这些缺口。

## Phase gates

### A. 基线与准入

采用 Responses-first + scoped extensions，按 request/item/part/tool/response/event/context 列出标准与扩展边界。确定 required/presence、origin、生命周期、保真与拒绝，消除旧 ADR 和当前源码冲突。设计文档完成不能将字段标为已实现。

### B. 已支持子集的正确性

先修 IR 约束丢失、静态/事件必填性、静默丢弃、严格 JSON、Schema 与派生 SDK view。每项先独立失败用例，再同步 semantic、requirements、lowering、codec。保留当前 Responses text 验收，不从零重写正常工作机制。

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

实施范围由 [current-focus](../implementation-plans/current-focus.md)记录，不由本路线图自动授予。本轮是文档/上游同步，不包含上述代码修复。
