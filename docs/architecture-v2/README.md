# OpenBridge Semantic Architecture v2

**设计基线：Responses-first 标准语义 + scoped extensions。** 当前实现仍为 Rust 语义库与离线验收，不是完整标准实现或可运行网关。

## 当前方向

Generation 主要参考 OpenAI Responses 的 request、ordered item/content、tool、reasoning、state 和 event 定义。Chat 及其他协议是目标映射，不以多协议最小交集限制 IR。Codex session/context 与特殊多模态通过有明确 owner、schema、来源和生命周期的扩展承载，不走任意 JSON/header 透传。

[IR 设计](semantic-ir.md)拥有结构与扩展准入；[主题化历史综合](../references/semantic-baseline.md)提供设计依据；[上游同步](../references/upstream-sync.md)固定官方语义、SDK 和 Codex 的来源版本。

## 处理模型

```text
Wire + trusted admission context
 -> Protocol Codec
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
- 当前源码只实现 Generation 的部分 Responses/Chat 语义、lowering 和纯 SSE。
- Responses 标准全景是目标；stateless text 是现有实施子集，不是长期 IR 表达力上限。
- 固定 Responses/Chat SDK gates 验证有限纯文本 JSON/SSE；hosted tools、state/WS 与真实 Provider 执行仍未实现。
- 后续先按[迁移基线](migration.md)修正核心闭合缺口，再按域推进。独立任务、topology/execution、credentials、MCP 和观测按各自获准切片实施，不创建通用插件框架。

## 文档所有权

- [semantic-ir.md](semantic-ir.md)：标准主干、整体内部表示、扩展 attachment、身份和 presence。
- [domain-model.md](domain-model.md)：task/model/profile/endpoint 等维度与运行时边界。
- [protocol-and-lowering.md](protocol-and-lowering.md)：codec、fidelity、固定 profile 和目标可表示性。
- [capability-model.md](capability-model.md)：标准可表达、模型支持、表示与执行能力分开。
- [invariants.md](invariants.md)：语义、扩展、安全与资源不变量。
- [execution-model.md](execution-model.md)：后续执行目标，不是当前已实现模块。
- [rust-layout.md](rust-layout.md)：职责布局方向，不复制 SDK 文件树。
- [migration.md](migration.md)：目标相对当前代码的差距和实施顺序。
- [responses-text-profile.md](responses-text-profile.md)：当前 Responses stateless text 的实现准入，不代表完整标准。
- [chat-text-profile.md](chat-text-profile.md)：同一 IR 的单候选 Chat 静态/流式映射与拒绝边界。
- [schema-profile.md](schema-profile.md)：请求/报告设置共享的 Schema 结构、strict/default、本地引用与预算准入。

既有 decisions 维护其当前有效规则，不添加完成日志或平行 schema；reasoning 的 owner/origin/finality 见[专项规则](decisions/0006-reasoning-ownership.md)。设计与历史来源有冲突时，依据当前需求及固定一手证据显式解决；来源快照不构成冻结设计的理由。

## 验收原则

独立 decode/encode 预期、IR 修改/删除、扩展来源隔离和 Static/Event 一致性是主要门槛。round trip、SDK 宽松解析或类型存在不证明完成。当前具体反例见[迁移缺口](migration.md#当前已知闭合缺口)，方法见[验收基线](../references/conformance-baseline.md)，当前切片见[current focus](../implementation-plans/current-focus.md)。
