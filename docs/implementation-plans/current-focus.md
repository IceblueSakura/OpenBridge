# 当前开发焦点

## P0：显式消息组与空 owner 的完整投影

**状态：按本片合同实施，分阶段验证提交，不 push。** 本片只处理现有 assistant message 与 function calls 的显式归属，以及该 owner 在 Responses 静态/事件投影中的存续；不是通用 Group/Turn 实现。方向与概念 owner 分别见 [next-goal](next-goal.md)、[semantic IR](../architecture-v2/semantic-ir.md)和[identity ADR](../architecture-v2/decisions/0002-task-ir-and-identities.md)。

### 可观察结果与要求

- 以 `ToolCall.message` 为唯一已声明的成员关系，提供 request/response 的只读消息组视图。组引用稳定 ItemId，成员顺序来自最终 items；视图不保存第二份成员清单，不按位置猜测调用归属，不创建 GroupId 或逻辑 turn 状态。没有关联的独立调用、reasoning、user message 和工具结果不自动入组。
- 消息组包含一个 assistant message owner 及其显式关联的连续 function calls；无调用的 assistant message 也是可识别的 owner。删除调用后视图重算；删除 owner、跨组穿插或改变 role 导致悬空/非法关系时拒绝，除非调用方显式修改关系。整组重排保留身份，不能把调用自动挂到新邻居。
- 静态与 reducer 对合法成员关系、refusal 与调用的互斥保持一致；在冲突已知时拒绝事件，而不是等到 terminal 才发现。此约束不把 message 完成等同于全部调用完成、response 完成或 turn 结束。
- Responses 编码不得因为 message 没有正文且有调用就删除已有 typed owner。保留空 `content`、phase、状态及独立 wire identity，静态 output、item-done 和 terminal output 必须一致；identity 碰撞检查也覆盖该 owner。
- 这是跨协议输出 item 保留行为的修正：原来省略的空 owner 会出现在 Responses JSON 的 output/input 中，与 SSE 所报告的 owner 一致；不新增 wire 字段或新协议扩展。标准 Responses 没有对应的 call→message 关联字段，重新解码时不伪造这个关联；若后续 Chat 单候选投影因此无法无损表达，明确拒绝，不靠邻接位置猜组。

### 独立失败用例与验证

1. 多个 Chat assistant 消息各有调用，组视图正确区分无正文与带正文 owner；请求与响应编辑后按最终身份/顺序导出，不吞并独立调用或 reasoning。独立构造 IR→wire 预期，不仅做 round trip。
2. 删除 owner、错误 role、调用跨组移动、悬空引用失败；整组移动、删除单个成员仍合法。stream 的 call→refusal 和 refusal→call 都在冲突事件被拒绝，之后不能恢复成功终态。
3. Chat calls-only response→Responses：静态 output 与完整 SSE 的重建结果一致，保留空 owner 与 call；用 phase/非成功 item 状态和重复 wire ID 反例证明 owner 不能按正文长度删除。
4. Responses wire 只有相邻 message/call 不等于报告了归属；不把拒绝转换改成隐式分组。保留现有有正文消息、不完整 response、工具 call/result、opaque replay 的独立回归。
5. 复用现有实际 Router smoke 与固定 SDK JSON/SSE 工具续轮 gate，不增加重复 Router。SDK 使用 synthetic key、loopback 与锁定依赖；必要时用独立 fixture 覆盖空 output message，不能仅以 Rust encoder 回读自证。

### 实施阶段

1. **消息组与语义验证**：先补红例，在 generation owner 下实现有界、只读关系视图，联动 request/response 及 reducer，保留现有语义主值和 wire 合同。通过聚焦检查和 Rust 基线后单独提交。
2. **Responses 投影闭合**：确认静态/事件反例后修复 owning codec/lowering，同步受影响的独立预期、Router/SDK 和 [Responses](../architecture-v2/responses-text-profile.md) / [Chat](../architecture-v2/chat-text-profile.md) 合同。完成基线及显式 SDK gate 后提交并清空本文件；next-goal 中的其他分组、attachment 与前缀依赖仍未完成。

每阶段按[开发指南](../development.md)执行 `cargo fmt -- --check`、`cargo test --locked --offline`、`cargo clippy --locked --offline --all-targets -- -D warnings`，并检查最终 diff、文档链接与 `git diff --check`。未改变 schema 字段无需无意义更新 OpenAPI；如发现需要新公共 carrier，则先重新明确合同与范围。

### 非目标与外部边界

不建立任意依赖图、统一签名类型、通用 GroupId/TurnId、自动工具执行/续轮、会话服务或新的重试/fallback。Reasoning 的 carrier 关系、跨 owner 的 opaque 依赖、设置/前缀依赖、Google/Anthropic codec 留待有具体合同的后续切片；不把只读消息组视图宣称为它们的实现。

本片优先离线和 synthetic SDK 验收。真实 Provider 请求仅在有独立验证价值时另建[受控计划](../probes.md)，明确目标、矩阵、请求/token 上限及脱敏输出，不因允许使用 key 而自动扩大调用范围，不复用旧 run 额度。
