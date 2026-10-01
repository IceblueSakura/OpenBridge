# 当前开发焦点

## P0：Responses reasoning replay 的唯一主值与依赖闭合

**状态：正在按本片合同实现。** 先以独立反例驱动闭合迁移，再进行离线与 synthetic SDK 验证，按阶段审阅提交，不 push。本片不延续此前 live run 的额度，也不证明任何能力已经完成。整体方向见[next-goal](next-goal.md)，设计依据见[semantic IR](../architecture-v2/semantic-ir.md)与[ADR 0006](../architecture-v2/decisions/0006-reasoning-ownership.md)。

### 选择理由与范围

当前 [ReasoningItem](../../src/semantic/task/generation/reasoning.rs)只有可读 parts 与 status；opaque 主值仍由 [FidelityRecords](../../src/protocol/fidelity.rs)持有，静态 codec 从中编码，事件路径再通过 `sync_replays` 转存。下一片应先把已有 Responses encrypted reasoning 的值权威、来源证明和变换规则分开，而不是在没有具体映射依据时扩建通用 Group/Turn 框架。

范围为现有 Responses `encrypted_content` 的 request/history、静态 response、SSE，以及已有显式 Chat reasoning carrier 的必要回归。保留 response outcome 与只读 pending-call continuation 的区分；本片不把 `Continuation::Unreported` 改为逻辑 turn 已结束，也不让 pending-call view 冒充完整 replay 要求。

### 可观察结果与目标合同

1. **唯一主值**：Responses opaque 值及其 partial/final 状态归属于 surviving typed reasoning owner，而非仅藏在 fidelity 中。只读语义访问、克隆、相等性与内容变换须能观察该值的存在、替换和删除；没有可读 summary 的 opaque-only item 仍可表达。保持 Responses 类型含义，不命名为任意厂商通用 token。
2. **证明与值分离**：来源/依赖记录只保存受信 scope、owner 绑定及有界摘要等校验事实，不保留第二份可恢复的 token。绑定须覆盖最终 opaque 值与本片已定义的 owner 依赖，不能让换 token 沿用旧证明。删除证明意味着无法满足所需回放准入，不意味着恢复或自动删除语义值。
3. **变换可验证**：删除 opaque 值或整个 owner 后，旧 source records 不得重新输出它；修改可读正文、part identity/order、owner status 或 opaque 值后，旧证明失效。保持 identity 且未改变依赖的独立 item 重排不应因 wire index 改变而串位；不凭空增加跨 item/group 约束。保留失效值时须拒绝 replay，调用方可显式移除该值或通过获准受信边界建立新的有效绑定。
4. **最终性分层**：分别判断值完整、owner 完成、response 终止与能否进入下一请求。Item-start 的 partial 值不能作为最终 replay；item-finish 的替换或移除以事件为权威。已完成 owner 不因 response 随后 incomplete 而自动失效；未完成 owner 不因 token 为 Final 而获得续轮资格。区分静态输出中报告部分内容与将其用于后续请求的准入，不用统一丢弃掩盖两者差异。
5. **来源与安全**：未知/不匹配 scope 不得 replay；业务 JSON 不能授信、改绑定或选择目标。现有 Gateway 的隔离与 credential 边界不变。本地摘要不是 issuer 签名验证；不解密、重签或制造 token。Opaque 值的 Debug/错误输出须脱敏；检查受影响的 response-wide dependency 摘要，不能依赖脱敏 Debug 导致 token-only 修改无法使旧 extras 失效。
6. **有界与单一计量**：opaque 与可读内容纳入明确的单值/聚合语义预算；不因移出 fidelity 而漏算，也不因 reducer 和 materialization 的多份表示重复计算同一逻辑值。保留独立的 raw JSON、SSE framing、partial state 和 HTTP body 限额。
7. **完整路径**：Static/Event materialization、IR 编辑、目标 lowering、下游交付及下一请求回放使用相同权威。Responses wire 字段和已有 Chat carrier 格式保持不变；未发布 Rust API 的替换同步所有调用方，不用 legacy alias 或新 schema 版本掩盖所有权迁移。

### 需要先建立的独立反例

以下是目标验收，不是已经执行失败的测试记录；先复用或扩充最低 owning layer 的独立预期，再实现：

- 可读内容相同但 opaque 值不同的输入，typed 语义不能忽略差异；清除 typed opaque 值后带着旧 fidelity 编码不得复活 token。
- 换 token、修改正文、改变 owner 状态、移植到另一 owner 或使用错误 scope，不能沿用旧回放证明；清除 source record 也不能把未知来源升级为可信来源。
- 两个独立 reasoning owner 重排保持关联；删除其中一个不损坏另一个，也不将 token 附着到旧数组位置。
- partial→final、final 替换、final 移除、opaque-only 和已完成 owner + incomplete response，分别验证静态与事件结果；request replay 拒绝不满足 owner/value 最终性的组合。
- truncated EOF、缺失必需 done、terminal snapshot 冲突、错误后继续输入，不得补造 final 值或成功终态。
- opaque 单值超限、可读与 opaque 合并超限、分片与替换计量；Debug/错误中无测试 token，token-only 修改仍使相关依赖记录失效。
- Responses response→客户端历史→下一 request 保留最终 token、wire identity 与工具 call/result 关联；普通 Chat 无 carrier 时拒绝，已有显式 carrier 保持原有作用域和拒绝边界。

### 实施顺序与涉及的 owner

1. **定稿最小 API 与依赖输入**：在现有 reasoning/replay owner 内明确值、阶段、来源/依赖证明各自唯一位置；核对[固定 Responses 来源](../references/upstream-sync.md)与现有 profile 对 partial 输出、history 和完整 response 的区别。若发现协议证据与上述最终性目标冲突，先明确冲突，不以代码现状或其他协议规则代替 Responses 合同。
2. **TDD 完成一次闭合迁移**：联动 [reasoning 类型](../../src/semantic/task/generation/reasoning.rs)、[验证](../../src/semantic/task/generation/validate.rs)、[reducer](../../src/semantic/task/generation/event.rs)、[fidelity](../../src/protocol/fidelity.rs)、[静态 reasoning codec](../../src/protocol/openai/reasoning.rs)、[event codecs](../../src/protocol/openai/events/mod.rs)与[lowering](../../src/lowering/generation.rs)。移除以 source token 为主值的路径；同步 adapter 的受信绑定、Chat carrier、examples 与测试。不要提交需要静默降级或双份权威才能工作的半迁移状态。
3. **补齐边界与端到端验收**：优先扩充 [reasoning](../../tests/semantic/reasoning.rs)、[reasoning boundary](../../tests/semantic/reasoning_boundary.rs)及[Responses SSE](../../tests/transport/responses_sse.rs)的独立用例；复用现有 Router/SDK 回环，只有新增独立价值时才增加 smoke。同步受影响的 Responses/Chat profile、protocol/lowering 文档与实施缺口，不保存完成日记。
4. **阶段提交与收尾**：后续执行时，每个可独立交付的阶段通过其受影响检查并审阅 diff 后再提交，不 push；本片验收闭合后清空 current-focus，未完成的 group、其他 attachment 与 turn 设计继续保留在 next-goal/缺口 owner，不宣称整个 P0 完成。

### 验证边界与退出条件

- 测试优先使用 **OpenAI Responses 协议**；独立 wire→IR、IR→wire、编辑及静态/事件预期不能仅靠 round trip 自证。已存在的 Chat carrier 只做受影响回归，不借本片扩展其准入。
- 聚焦检查后执行 `cargo fmt -- --check`、`cargo test --locked --offline`、`cargo clippy --locked --offline --all-targets -- -D warnings` 和 `git diff --check`；完整命令与依赖约束见[开发指南](../development.md)。
- 固定 OpenAI SDK 的 Responses JSON/SSE 三轮和 SDK→实际 Router 回环作为单独显式 gate，使用 synthetic key、受控 loopback、锁定依赖，不继承真实凭据或自动 retry。
- 如需真实验收，另建[受控 probe 计划](../probes.md)，当次明确目标、请求矩阵、预算与脱敏界限。纯文本或可读 reasoning 成功不证明 opaque replay；不为获得 token 放宽准入、伪造内容或扩大预算。真实结果不写入本文，live gate 不替代独立变换和失败用例。
- 退出条件：静态、事件和 history 的 opaque 主值均只有一个 typed owner；旧证明不能恢复/授权新值；最终性、隔离、资源与敏感输出反例闭合；调用方和受影响文档一致。未变的 HTTP wire 无需无意义修改 OpenAPI，若确需改变公共 carrier 则先重新明确范围并同步所有公共合同。

### 非目标

不接入 Google/Anthropic codec，不实现通用 signature/动态扩展注册表、GroupId/TurnId 框架或任意依赖图；不增加远端 state、pause_turn、会话存储、自动工具执行/续轮、跨账户回放或 retry/fallback；不改变缓存/usage/媒体能力、路由、认证、Provider 目录或 OAuth 顺序。其他 attachment 的扩展应在本片提供可靠值/证明边界后，依据选定协议合同另立切片。
