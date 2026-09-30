# OpenBridge Semantic Architecture v2

**设计基线：Responses-first 标准语义 + scoped extensions。** 当前 `main` 以原 `semantic-v2` 的 Rust 语义库与最小 loopback 文本网关为基线，独立推进产品目标，不以旧版迁移或功能对等为目标；具有 synthetic HTTP/SDK 全链验收，但不是完整标准实现或生产就绪服务。

## 产品目标与阶段判据

最终产品是**多模态、Chat Completions / Responses 兼容、Agent 友好、缓存亲和性强的 IR 化网关**。当前以仅文本 Generation 验证整体流程，验证后再扩展 Provider 与多模态；离线库是实现基础，不是最终交付边界。

- **IR 化与双协议兼容**：请求、响应和事件都经同一语义权威；同协议无旁路，跨协议按可表示性映射或明确拒绝，不以 Chat 最小交集定义 IR。
- **统一语义与显式边界适配**：Core 拥有充分的 task/context/delivery 语义，不建立厂商分支。client/Provider adapters 组合公共协议，通过映射、派生 view 校验、准确推导、逐字段兼容默认值与有来源约束的 fidelity 吸收差异。缺省零是兼容结果，不是 Provider 实测报告；默认值不得掩盖非法值、恢复删除值或编造成功终态。机制见 [ADR 0008](decisions/0008-stable-core-and-vendor-adapters.md)。
- **Agent 友好**：工具定义/选择、调用身份、原始参数、结果回传、reasoning/派生 view 的续轮回放和流式非成功边界一致；不等于网关代替 Agent 执行所有工具，也不以单个 SDK gate 宣称全面 Agent 兼容。
- **缓存亲和性强**：尽量保持合法续轮的稳定前缀、工具/Schema 顺序、cache affinity 与来源约束；session/cache/thread/turn 各有 owner，不为缓存复活被删除语义，不跨认证所有权重放 opaque state。稳定投影、实际缓存命中和成本/延迟效果是不同验收层。
- **可扩展多模态**：保留标准媒体、资源和独立任务的正确所有权；文本先行不授权把未来媒体语义压成字符串，也不要求先实现未来所有任务才验证网关主链。

阶段退出看选定文本场景的入口、IR、lowering、upstream adapter、响应交付和多轮回放是否连成可验收路径，不看字段数或测试数。推进顺序与具体门槛见[下一步目标](../implementation-plans/next-goal.md)，现有完成度见[当前能力与边界](../implementation-status/generation.md)。

## 当前方向

Generation 主要参考 OpenAI Responses 的 request、ordered item/content、tool、reasoning、state 和 event 定义。Chat 及其他协议是目标映射，不以多协议最小交集限制 IR。Codex session/context 与特殊多模态通过有明确 owner、schema、来源和生命周期的扩展承载，不走任意 JSON/header 透传。

[IR 设计](semantic-ir.md)拥有结构与扩展准入；[主题化历史综合](../references/semantic-baseline.md)提供设计依据；[上游同步](../references/upstream-sync.md)固定官方语义、SDK 和 Codex 的来源版本。

## 处理模型

```text
Wire + trusted admission context
 -> Client / Provider Adapter over shared Protocol Codec
 -> Responses-oriented Generation semantics + context/delivery/extensions
 -> Validation / Trusted Transform
 -> Requirements
 -> Fixed Candidate Representability / Lowering
 -> Protocol Codec
 -> Wire
```

没有 Native 语义旁路，也没有 Bridge 领域对象。纯 codec/lowering 不访问 credential、registry 或网络；上下文扩展可表达 session 等事实，但不能包含选定路由、socket、真实凭据或任意执行脚本。

## 设计与实现边界

- 旧运行时已[归档](../archive.md)，不要求功能对等或保留旧 crate path。
- 当前源码实现 Generation 的部分 Responses/Chat 语义、标准 context、显式 adapters、lowering、纯 SSE、固定 topology、caller-driven execution 与最小认证 HTTP 入口。
- Responses 标准全景是目标；stateless text 是现有实施子集，不是长期 IR 表达力上限。
- 固定 SDK 既有 codec fixture gates，也有经实际 Router 和 synthetic HTTP Provider 的双协议 JSON/SSE 续轮 gate；hosted tools、state/WS 与生产级保障仍未实现。库级 live 证据不能代替新服务入口的外部验收。
- 当前分层完成度与具体缺口由[能力与边界](../implementation-status/generation.md)维护，推进顺序只由[下一步目标](../implementation-plans/next-goal.md)维护。现有纯文本基线不等于完整标准；未来任务或工具执行也不是无限延迟最小执行设计的前置条件。

## 文档所有权

- [semantic-ir.md](semantic-ir.md)：标准主干、整体内部表示、扩展 attachment、身份和 presence。
- [domain-model.md](domain-model.md)：task/model/profile/endpoint 等维度与运行时边界。
- [protocol-and-lowering.md](protocol-and-lowering.md)：codec、fidelity、固定 profile 和目标可表示性。
- [capability-model.md](capability-model.md)：标准可表达、模型支持、表示与执行能力分开。
- [invariants.md](invariants.md)：语义、扩展、安全与资源不变量。
- [execution-model.md](execution-model.md)：执行职责和交付合同；现有 library、最小 HTTP 接线与生产边界见当前架构。
- [rust-layout.md](rust-layout.md)：职责布局方向，不复制 SDK 文件树。
- [Generation 当前能力与边界](../implementation-status/generation.md)：分层完成度、产品目标相对当前代码的差距、可复现反例与边界；不维护旧版迁移清单或重复实施顺序。
- [responses-text-profile.md](responses-text-profile.md)：当前 Responses stateless text 的实现准入，不代表完整标准。
- [chat-text-profile.md](chat-text-profile.md)：同一 IR 的单候选 Chat 静态/流式映射与拒绝边界。
- [schema-profile.md](schema-profile.md)：请求/报告设置共享的 Schema 结构、strict/default、本地引用与预算准入。

最小 HTTP 入口、启动固定候选与 body handoff 边界见 [ADR 0009](decisions/0009-minimal-http-text-gateway.md)。既有 decisions 维护其当前有效规则，不添加完成日志或平行 schema；reasoning 的 owner/origin/finality 见[专项规则](decisions/0006-reasoning-ownership.md)。设计与历史来源有冲突时，依据当前需求及固定一手证据显式解决；来源快照不构成冻结设计的理由。

## 验收原则

独立 decode/encode 预期、IR 修改/删除、扩展来源隔离和 Static/Event 一致性是主要门槛。round trip、SDK 宽松解析或类型存在不证明完成。已准入分支的独立反例入口见[正确性边界](../implementation-status/generation.md#已闭合的正确性边界)，方法见[验收基线](../references/conformance-baseline.md)，当前切片见[current focus](../implementation-plans/current-focus.md)。
